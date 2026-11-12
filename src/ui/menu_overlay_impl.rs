use crate::run::state::GLOBAL::db;
use crate::{
    abspath::AbsPath,
    config::StashPaneKind,
    db::DbTable,
    run::{
        FsAction,
        item::{PathItem, short_display},
        state::{GLOBAL, MenuPrompt, STACK, TASKS, TOAST, ToastFlags, ToastStyle},
    },
};

use cba::{bath::PathExt, blaim};
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
    NewFolder,
    Rename,
    #[strum(serialize = "Jump to")]
    Jump,
    #[strum(serialize = "Extract to")]
    Extract,
    #[strum(serialize = "Set alias")]
    SetAlias,
    Execute,
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

/// The extract prompt for `path`: the input is prepopulated with the
/// archive's sibling directory named after its stem (`foo.tar.gz` ->
/// `foo.tar`), so accepting immediately extracts beside the archive.
pub fn extract_prompt_for(path: &AbsPath) -> MenuPrompt {
    let dest = path._parent().join(path.file_stem().unwrap_or_default());
    MenuPrompt::new(PromptKind::Extract).initial(dest.to_string_lossy())
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

    /// The selected items, or the cursor item (the cwd while the cursor is
    /// disabled) when nothing is selected.
    pub fn target_paths(
        &self,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> Vec<AbsPath> {
        let mut paths: Vec<AbsPath> = state.map_selections_to_vec(|_, item| item.path.clone());
        if paths.is_empty() {
            paths.push(self.target_path(state));
        }
        paths
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
        print_flavor: bool,
    ) -> OverlayEffect {
        match prompt {
            PromptKind::New => {
                let current_item_parent = self.target_parent(state);
                let input = self.prompt.input.value();
                let into_dir = Path::new(&input).ends_in_separator();
                let dest = AbsPath::new_unchecked(Path::new(&input).abs(&current_item_parent));

                TASKS::spawn("create", async move {
                    // parents first, then an exclusive claim: an existing
                    // name is reported as skipped instead of truncated or
                    // silently merged
                    let reserved = if into_dir {
                        blaim::reserve_dir_all(&dest, blaim::ClaimPolicy::Strict).map(|_| ())
                    } else {
                        blaim::reserve_file_all(&dest, blaim::ClaimPolicy::Strict).map(|_| ())
                    };
                    match reserved {
                        Ok(()) => {
                            TOAST::push_with_flag(
                                ToastStyle::Success,
                                "New: ",
                                [short_display(&dest)],
                                ToastFlags::SUSTAINED,
                            );
                        }
                        Err(blaim::ClaimError::Taken) => TOAST::push_skipped(),
                        Err(e) => {
                            log::error!("Failed to create {dest:?}: {e:?}");
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to create: ",
                                [short_display(&dest)],
                            );
                        }
                    }
                });
            }
            PromptKind::NewFolder => {
                let current_item_parent = self.target_parent(state);
                let input = self.prompt.input.value();
                let dest = AbsPath::new_unchecked(Path::new(&input).abs(current_item_parent));
                let cd = Path::new(&input).ends_in_separator();

                TASKS::spawn("mkdir", async move {
                    // parents first, then an atomic claim on the final name:
                    // a taken name is reported instead of merged into
                    match blaim::reserve_dir_all(&dest, blaim::ClaimPolicy::default()) {
                        Ok(reserved) => {
                            if let Some(path) = reserved.into_path() {
                                TOAST::push_with_flag(
                                    ToastStyle::Success,
                                    "New: ",
                                    [short_display(&path)],
                                    ToastFlags::SUSTAINED,
                                );
                                if cd {
                                    GLOBAL::send_action(FsAction::Jump(vec![path]));
                                }
                            }
                        }
                        Err(blaim::ClaimError::Taken) => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Already exists: ",
                                [short_display(&dest)],
                            );
                        }
                        Err(blaim::ClaimError::Io(e)) => {
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

                    let handle_claim_err = |e: blaim::ClaimError| match e {
                        blaim::ClaimError::Taken => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Already exists: ",
                                [short_display(&dest)],
                            );
                        }
                        blaim::ClaimError::Io(e) => {
                            log::error!("Failed to rename target {dest:?}: {e}");
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to rename: ",
                                [short_display(&old_path)],
                            );
                        }
                    };

                    let replaced = if is_dir {
                        match blaim::replace_dir(&old_path, &dest, blaim::ClaimPolicy::Strict) {
                            Ok(None) => Ok(()),
                            Ok(Some(claim)) => {
                                claim.rollback();
                                Err(std::io::Error::other(
                                    "cross-device rename requires transfer engine",
                                ))
                            }
                            Err(e) => return handle_claim_err(e),
                        }
                    } else {
                        match blaim::replace_file(&old_path, &dest, blaim::ClaimPolicy::Strict) {
                            Ok(None) => Ok(()),
                            Ok(Some(claim)) => {
                                claim.rollback();
                                Err(std::io::Error::other(
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
            PromptKind::Jump => {
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
            PromptKind::Execute => {
                let input = self.prompt.input.value();
                if input.trim().is_empty() {
                    return OverlayEffect::None;
                }
                GLOBAL::send_action(if print_flavor {
                    FsAction::ExecDetached(input)
                } else {
                    FsAction::ExecuteWait(input)
                });
            }
            PromptKind::Extract => {
                let input = self.prompt.input.value();
                if input.trim().is_empty() {
                    return OverlayEffect::None;
                }
                let source = self.target_path(state);
                let dest = AbsPath::new_unchecked(Path::new(&input).abs(self.target_parent(state)));
                let label = short_display(&source);

                TASKS::spawn("extract", async move {
                    if let Err(e) = crate::unzip::extract_to(&source, dest.as_path()) {
                        log::error!("Failed to extract {}: {e}", source.to_string_lossy());
                        TOAST::push(ToastStyle::Error, "Failed to extract: ", [label]);
                    }
                });
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
