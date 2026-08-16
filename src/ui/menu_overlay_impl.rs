use crate::{
    abspath::AbsPath,
    config::StashPaneKind,
    db::DbTable,
    fs::{auto_dest, create_all, rename},
    run::{
        FsAction,
        item::{PathItem, short_display},
        state::{GLOBAL, MenuPrompt, STACK, TASKS, TOAST, ToastStyle},
    },
};

use cba::bath::{PathExt, RenamePolicy, auto_dest_for_src};
use matchmaker::{
    nucleo::{Color, Span},
    render::MMState,
    ui::{Overlay, OverlayEffect},
};
use std::path::Path;

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

/// Whether `path` is the process's current working directory. The logical
/// paths are compared first, falling back to canonical comparison in case the
/// cwd was reached through a symlink. Must be called while `path` still
/// exists.
fn current_dir_matches(path: &AbsPath) -> bool {
    let cwd = match std::env::current_dir() {
        Ok(c) => c,
        Err(_) => return false,
    };
    if cwd.as_path() == path.as_path() {
        return true;
    }
    match (cwd.canonicalize(), path.as_path().canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
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
                let input_path = Path::new(&input);
                let dest = auto_dest(input_path, &current_item_parent); // replaced if input is absolute
                let dest_slice = [dest];

                TASKS::spawn(async move {
                    match create_all(&dest_slice).await {
                        Ok(_) => {
                            let dest_path = match &dest_slice[0] {
                                Ok(p) | Err(p) => p,
                            };
                            TOAST::push(ToastStyle::Success, "New: ", [short_display(dest_path)]);
                        }
                        Err(_) => {
                            let dest_path = match &dest_slice[0] {
                                Ok(p) | Err(p) => p,
                            };
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
                let input_path = Path::new(&input);
                let dest = AbsPath::new_unchecked(input_path.abs(current_item_parent));
                let cd = input.ends_with(std::path::MAIN_SEPARATOR);

                TASKS::spawn(async move {
                    match std::fs::create_dir_all(&dest) {
                        Ok(_) => {
                            TOAST::push(ToastStyle::Success, "New: ", [short_display(&dest)]);
                            if cd {
                                GLOBAL::send_action(FsAction::Jump(vec![dest.into()]));
                            }
                        }
                        Err(_) => {
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
                let dest = AbsPath::new_unchecked(
                    auto_dest_for_src(
                        &old_path,
                        self.prompt.input.value(),
                        &RenamePolicy::default(),
                    )
                    .abs(old_path.parent().unwrap()),
                );

                if dest == old_path {
                    TOAST::push_skipped();
                    return OverlayEffect::None;
                }

                // Snapshot the picker and process state before the move: the
                // old path stops existing, and STACK is thread-local — the
                // spawned task can only react through GLOBAL::send_action.
                let renames_cwd = STACK::cwd().as_ref() == Some(&old_path);
                let renames_process_cwd = current_dir_matches(&old_path);

                TASKS::spawn(async move {
                    match rename(&old_path, &dest).await {
                        Ok(_) => {
                            let new_display = dest.to_string_lossy().to_string().into();
                            TOAST::pair(
                                ToastStyle::Success,
                                "Renamed: ",
                                short_display(&old_path),
                                new_display,
                            );
                            // the picker stood in the renamed directory: follow it;
                            // otherwise the watcher's rename event refreshes the
                            // listing, so no explicit reload is needed
                            if renames_cwd {
                                GLOBAL::send_action(FsAction::Jump(vec![dest.clone().into()]));
                            }
                            if renames_process_cwd {
                                // the process's own working directory moved: follow
                                // it so relative paths and spawned commands resolve
                                if let Err(e) = std::env::set_current_dir(&dest) {
                                    log::error!(
                                        "Failed to follow the renamed working directory {}: {e}",
                                        dest.to_string_lossy()
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
                    let pool = GLOBAL::db();
                    let tail = alias.clone();
                    let path = path.clone();
                    TASKS::spawn(async move {
                        match kind {
                            StashPaneKind::Transient => {
                                crate::run::stash::mem_set_tail(&name, &path, &tail);
                            }
                            _ => match pool.get_conn(DbTable::stashes).await {
                                Ok(mut conn) => {
                                    if let Err(e) = conn.set_stash_tail(&name, &path, &tail).await {
                                        log::error!("Error setting stash tail: {e}");
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error getting connection: {e}");
                                }
                            },
                        }
                        GLOBAL::send_action(FsAction::Reload);
                    });
                } else {
                    let pool = GLOBAL::db();
                    let table = if path.is_dir() {
                        DbTable::dirs
                    } else {
                        DbTable::files
                    };

                    pool.set_path_alias(path.clone(), alias.clone(), table);
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
