use crate::run::state::GLOBAL::db;
use crate::{
    abspath::AbsPath,
    config::StashPaneKind,
    db::DbTable,
    run::{
        FsAction,
        item::{PathItem, short_display},
        state::{GLOBAL, MenuPrompt, STACK, TASKS, TOAST, ToastStyle},
    },
};

use cba::{bath::PathExt, claim as cba_claim};
use matchmaker::{
    nucleo::{Color, Span},
    render::MMState,
    ui::{Overlay, OverlayEffect},
};
use std::path::{Path, PathBuf};

use super::menu_overlay::MenuOverlay;

#[derive(Debug, strum_macros::Display, Clone, Copy)]
pub enum PromptKind {
    New,
    #[strum(serialize = "New folder")]
    NewDir,
    Rename,
    #[strum(serialize = "Go to")]
    Goto,
    #[strum(serialize = "Set alias")]
    SetAlias,
}

/// The rename prompt for `path`: the input is prepopulated with the full path
/// and the cursor sits at the end of the file stem, so typing replaces the
/// name while the extension survives.
pub fn rename_prompt_for(path: &AbsPath) -> MenuPrompt {
    let filename = path.to_string_lossy().into_owned();
    let cursor = path
        .with_file_name(path.file_stem().unwrap_or_default())
        .to_string_lossy()
        .len();
    MenuPrompt {
        kind: PromptKind::Rename,
        title: "Rename".to_string(),
        initial: filename,
        cursor,
    }
}

/// If the process's current working directory is inside `path` (or equals
/// `path`), returns the relative path from `path` to the cwd.
/// Must be called while `path` still exists.
pub(crate) fn current_dir_suffix(path: &AbsPath) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    if let Ok(rel) = cwd.strip_prefix(path.as_path()) {
        return Some(rel.to_path_buf());
    }
    match (cwd.canonicalize(), path.as_path().canonicalize()) {
        (Ok(can_cwd), Ok(can_path)) => can_cwd.strip_prefix(&can_path).ok().map(Path::to_path_buf),
        _ => None,
    }
}

impl MenuOverlay {
    /// The current item under the cursor, or the stack cwd when the cursor is
    /// disabled. The picker state cannot change while the menu overlay is
    /// open (input is intercepted), so this equals the state at menu open.
    pub fn target_path(
        &self,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> AbsPath {
        crate::run::register::resolve_target(state, true)
            .or_else(STACK::cwd)
            .unwrap_or_else(STACK::_cwd)
    }

    pub fn target_parent(
        &self,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> AbsPath {
        state
            .picker_ui
            .current_indexed()
            .map(|(_, p)| p.path._parent())
            .or_else(STACK::cwd)
            .unwrap_or_else(STACK::_cwd)
    }

    pub fn on_prompt_accept(
        &mut self,
        prompt: PromptKind,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        match prompt {
            PromptKind::New => {
                let current_item_parent = self.target_parent(state);
                let input = self.prompt.input.value();
                let dest = crate::utils::path::auto_dest(&input, &current_item_parent); // replaced if input is absolute

                TASKS::spawn("create", async move {
                    // parents first, then an exclusive claim: an existing
                    // name is reported as skipped instead of truncated or
                    // silently merged
                    let claimed = match &dest {
                        Ok(file) => {
                            cba_claim::reserve_file_all(file, cba_claim::ClaimPolicy::Strict)
                                .map(|_| ())
                        }
                        Err(dir) => cba_claim::reserve_dir_all(dir, cba_claim::ClaimPolicy::Strict)
                            .map(|_| ()),
                    };
                    let dest_path = match &dest {
                        Ok(p) | Err(p) => p,
                    };
                    match claimed {
                        Ok(()) => {
                            TOAST::push(ToastStyle::Success, "New: ", [short_display(dest_path)]);
                        }
                        Err(cba_claim::ClaimError::Taken) => TOAST::push_skipped(),
                        Err(e) => {
                            log::error!("Failed to create {dest_path:?}: {e:?}");
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to create: ",
                                [short_display(dest_path)],
                            );
                        }
                    }
                });
            }
            PromptKind::NewDir => {
                let current_item_parent = self.target_parent(state);
                let input = self.prompt.input.value();
                let dest = AbsPath::new_unchecked(Path::new(&input).abs(current_item_parent));
                let cd = input.ends_with(std::path::MAIN_SEPARATOR);

                TASKS::spawn("mkdir", async move {
                    // parents first, then an atomic claim on the final name:
                    // a taken name is reported instead of merged into
                    match cba_claim::reserve_dir_all(&dest, cba_claim::ClaimPolicy::default()) {
                        Ok(reserved) => {
                            if let Some(path) = reserved.into_path() {
                                TOAST::push(ToastStyle::Success, "New: ", [short_display(&path)]);
                                if cd {
                                    GLOBAL::send_action(FsAction::Jump(vec![path]));
                                }
                            }
                        }
                        Err(cba_claim::ClaimError::Taken) => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Already exists: ",
                                [short_display(&dest)],
                            );
                        }
                        Err(cba_claim::ClaimError::Io(e)) => {
                            log::error!("Failed to create {dest:?}: {e}");
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to create: ",
                                [short_display(&dest)],
                            );
                        }
                    }
                });
            }
            PromptKind::Rename => {
                let old_path = self.target_path(state);
                if old_path.file_name().is_none() {
                    return OverlayEffect::None;
                }
                let input = self.prompt.input.value();
                let dest =
                    AbsPath::new_unchecked(Path::new(&input).abs(old_path.parent().unwrap()));

                if dest == old_path {
                    TOAST::push_skipped();
                    return OverlayEffect::None;
                }

                // Snapshot any relative process cwd before the move: the old path stops
                // existing on disk.
                let renames_process_cwd = current_dir_suffix(&old_path);

                TASKS::spawn("rename", async move {
                    // reserve the target first: a taken name fails the
                    // rename instead of POSIX-replacing whatever is there;
                    // directories must claim a directory placeholder, or
                    // rename(2) fails with ENOTDIR against the file claim;
                    // missing destination parents are invented by the claim
                    // and repaid whenever the reservation is rolled back
                    let is_dir = std::fs::symlink_metadata(&old_path)
                        .map(|m| m.is_dir())
                        .unwrap_or(false);

                    let handle_claim_err = |e: cba_claim::ClaimError| match e {
                        cba_claim::ClaimError::Taken => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Already exists: ",
                                [short_display(&dest)],
                            );
                        }
                        cba_claim::ClaimError::Io(e) => {
                            log::error!("Failed to rename target {dest:?}: {e}");
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to rename: ",
                                [short_display(&old_path)],
                            );
                        }
                    };

                    let replaced = if is_dir {
                        match cba_claim::replace_dir(&old_path, &dest, cba_claim::ClaimPolicy::Strict) {
                            Ok(None) => Ok(()),
                            Ok(Some(claim)) => {
                                claim.rollback();
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    "cross-device rename requires transfer engine",
                                ))
                            }
                            Err(e) => return handle_claim_err(e),
                        }
                    } else {
                        match cba_claim::replace_file(&old_path, &dest, cba_claim::ClaimPolicy::Strict) {
                            Ok(None) => Ok(()),
                            Ok(Some(claim)) => {
                                claim.rollback();
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    "cross-device rename requires transfer engine",
                                ))
                            }
                            Err(e) => return handle_claim_err(e),
                        }
                    };

                    match replaced {
                        Ok(()) => {
                            let new_display = dest.to_string_lossy().to_string().into();
                            TOAST::pair(
                                ToastStyle::Success,
                                "Renamed: ",
                                short_display(&old_path),
                                new_display,
                            );
                            if let Some(rel) = renames_process_cwd {
                                // the process's own working directory (or an ancestor) moved:
                                // follow it so relative paths and spawned commands resolve
                                let new_cwd = dest.as_path().join(rel);
                                if let Err(e) = std::env::set_current_dir(&new_cwd) {
                                    log::error!(
                                        "Failed to follow the renamed working directory {}: {e}",
                                        new_cwd.display()
                                    );
                                    TOAST::notice(
                                        ToastStyle::Warning,
                                        "Renamed the working directory; restart to refresh it.",
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to rename {} to {}: {e}",
                                old_path.to_string_lossy(),
                                dest.to_string_lossy()
                            );
                            // the placeholder and invented ancestors were rolled back
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to rename: ",
                                [short_display(&old_path)],
                            );
                        }
                    }
                });
            }
            PromptKind::Goto => {
                let input = self.prompt.input.value();
                let input_path = Path::new(&input);
                if input_path.as_os_str().is_empty() {
                    return OverlayEffect::None;
                }
                // relative paths resolve against the current directory
                let dest = AbsPath::new_unchecked(input_path.abs(STACK::_cwd()));

                if dest.is_dir() {
                    GLOBAL::send_action(FsAction::Jump(vec![dest.into()]));
                } else {
                    TOAST::msg(
                        vec![
                            Span::styled(dest.to_string_lossy().to_string(), Color::Red),
                            Span::raw(" is not a valid directory!"),
                        ],
                        false,
                    );
                }
            }
            PromptKind::SetAlias => {
                let path = self.target_path(state);
                let alias = self.prompt.input.value();

                // in a stash pane this edits the entry's tail column instead
                let stash_name = STACK::with_current(|p| match p {
                    crate::run::FsPane::Stash { stash_name, .. } => Some(stash_name.clone()),
                    _ => None,
                });

                if let Some(name) = stash_name {
                    let kind = GLOBAL::cfg()
                        .panes
                        .stashes
                        .get(&name)
                        .map(|s| s.kind)
                        .unwrap_or_default();
                    let tail = alias.clone();
                    let path = path.clone();

                    if kind == StashPaneKind::Transient {
                        // in-memory stash: nothing async needed
                        crate::run::stash::mem_set_tail(&name, &path, &tail);
                        GLOBAL::send_action(FsAction::Reload);
                    } else {
                        TASKS::spawn("set alias", async move {
                            match db().get_conn(DbTable::stashes).await {
                                Ok(mut conn) => {
                                    if let Err(e) = conn.set_stash_tail(&name, &path, &tail).await {
                                        log::error!("Error setting alias: {e}");
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error getting connection: {e}");
                                }
                            }
                            GLOBAL::send_action(FsAction::Reload);
                        });
                    }
                } else {
                    let table = if path.is_dir() {
                        DbTable::dirs
                    } else {
                        DbTable::files
                    };

                    db().set_path_alias(path.clone(), alias.clone(), table);
                }

                if alias.is_empty() {
                    TOAST::push(
                        ToastStyle::Normal,
                        "Alias cleared: ",
                        [short_display(&path)],
                    );
                } else {
                    TOAST::push(ToastStyle::Success, "Alias set: ", [short_display(&path)]);
                }
            }
        }

        self.prompt_kind = None;
        self.prompt.on_disable();
        OverlayEffect::Disable
    }
}
