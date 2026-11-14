use std::path::{Path, PathBuf};

use cba::{bath::PathExt, blaim};
use matchmaker::{
    nucleo::{Color, Span},
    render::MMState,
    ui::{Overlay, OverlayEffect, Rect},
};

use crate::{
    abspath::AbsPath,
    config::StashPaneKind,
    db::DbTable,
    menu::{MenuEvaluationContext, MenuSnapshot, MenuStrategy},
    run::{
        FsAction, FsPane,
        item::{PathItem, short_display},
        queue::QUEUE,
        reload::fs_reload,
        state::{
            GLOBAL, GLOBAL::db, MenuCommandPaths, MenuPrompt, MenuPromptTarget, PromptKind, STACK,
            STORE, TASKS, TOAST, ToastFlags, ToastStyle, lessfilter_cfg,
        },
    },
};

use super::{
    MenuOverlay,
    items::{MenuItem, MenuOutcome},
};

/// A prompt open over the menu: its kind, the paths snapshotted with it, and
/// whether cancelling returns to the menu.
#[derive(Debug, Clone)]
pub(super) struct ActivePrompt {
    pub kind: PromptKind,
    pub paths: MenuPromptTarget,
    pub return_menu: bool,
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
    MenuPrompt::new(PromptKind::Rename)
        .title("Rename")
        .initial(filename)
        .cursor(cursor)
}

/// The extract prompt for `path`: the input is prepopulated with the
/// archive's sibling directory named after its stem (`foo.tar.gz` ->
/// `foo.tar`), so accepting immediately extracts beside the archive.
pub(super) fn extract_prompt_for(path: &AbsPath) -> MenuPrompt {
    let dest = path._parent().join(path.file_stem().unwrap_or_default());
    MenuPrompt::new(PromptKind::Extract).initial(dest.to_string_lossy())
}

pub(super) fn resolve_prompt_target(state: &mut MMState<'_, PathItem, ()>) -> AbsPath {
    crate::run::register::resolve_target(state, true)
        .or_else(STACK::cwd)
        .unwrap_or_else(STACK::_cwd)
}

pub(super) fn resolve_prompt_parent(state: &mut MMState<'_, PathItem, ()>) -> AbsPath {
    state
        .picker_ui
        .current_indexed()
        .map(|(_, p)| p.path._parent())
        .or_else(STACK::cwd)
        .unwrap_or_else(STACK::_cwd)
}

pub(crate) fn capture_prompt_target(state: &mut MMState<'_, PathItem, ()>) -> MenuPromptTarget {
    MenuPromptTarget {
        target: resolve_prompt_target(state),
        parent: resolve_prompt_parent(state),
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

/// The selected items, or the fallback target when nothing is selected.
fn snapshot_targets(snapshot: MenuSnapshot) -> (AbsPath, Vec<AbsPath>) {
    let path = snapshot.fallback;
    let targets = if snapshot.selected.is_empty() {
        vec![path.clone()]
    } else {
        snapshot.selected
    };
    (path, targets)
}

/// OpenWith creates the app pane right there: the selected items (or the
/// cursor item when nothing is selected) preload it, then the pane reloads —
/// the same bookkeeping as `FsAction::App`, done inline since the selection is
/// only known here.
fn open_with(
    snapshot: MenuSnapshot,
    state: &mut MMState<'_, PathItem, ()>,
) -> OverlayEffect {
    let (_, files) = snapshot_targets(snapshot);

    let displays: Vec<_> = files.iter().map(|p| short_display(p)).collect();

    // save input, then switch to the app pane and reload
    let (content, index) = state.get_content_and_index();
    STACK::save_input(content, index);
    STACK::push(FsPane::new_apps(files));
    fs_reload(state, true, false);
    // after the switch: entering a pane clears the toasts of the one left
    TOAST::push(ToastStyle::Normal, "Opening: ", displays);
    OverlayEffect::Disable
}

impl MenuOverlay {
    pub(super) fn set_prompt(
        &mut self,
        mut prompt: MenuPrompt,
        state: &mut MMState<'_, PathItem, ()>,
    ) {
        let paths = prompt
            .target
            .take()
            .unwrap_or_else(|| capture_prompt_target(state));
        self.active_prompt = Some(ActivePrompt {
            kind: prompt.kind,
            paths,
            return_menu: prompt.return_menu,
        });
        if !prompt.title.is_empty() {
            self.prompt.input.config.frame.title = prompt.title;
        }
        self.prompt.on_enable(&Rect::default(), state);

        if !prompt.initial.is_empty() {
            self.prompt.input.set_value(prompt.initial);
            self.prompt
                .input
                .inner
                .set(Option::<String>::None, prompt.cursor as u16);
        }
    }

    pub fn accept(
        &mut self,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let Some(worker) = self.worker.as_ref() else {
            return OverlayEffect::Disable;
        };
        let Some((_, entry)) = worker.get_nth_indexed(self.results.index()) else {
            return OverlayEffect::Disable;
        };
        let item = entry.item.clone();
        self.execute(item, state)
    }

    /// Run `item` on the current target, opening a prompt when the item
    /// returns one.
    pub(super) fn execute(
        &mut self,
        item: MenuItem,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let snapshot = MenuSnapshot::capture(state);
        if let MenuItem::Custom { action, .. } = item {
            return self.run_custom(&action, snapshot);
        }
        // the listing can predate a cursor move (ReloadMenu) or a mount change:
        // re-check the listing precondition against live state
        if let Err(msg) = item.check(&snapshot) {
            TOAST::notice(ToastStyle::Error, msg);
            return OverlayEffect::None;
        }
        match item {
            MenuItem::OpenWith => open_with(snapshot, state),
            item => self.run_action(item, snapshot, state),
        }
    }

    /// Apply a resolved item: open its prompt, keep the menu, or close it.
    fn run_action(
        &mut self,
        item: MenuItem,
        snapshot: MenuSnapshot,
        state: &mut MMState<'_, PathItem, ()>,
    ) -> OverlayEffect {
        let (path, targets) = snapshot_targets(snapshot);
        match item.action(path, targets) {
            MenuOutcome::Prompt(prompt) => {
                self.set_prompt(prompt, state);
                OverlayEffect::None
            }
            MenuOutcome::Stay => OverlayEffect::None,
            MenuOutcome::Close => OverlayEffect::Disable,
        }
    }

    /// Run a custom action on the target items (the current selection, or
    /// the target item when nothing is selected).
    fn run_custom(
        &mut self,
        key: &str,
        snapshot: MenuSnapshot,
    ) -> OverlayEffect {
        let Some(action) = self.actions.get(key) else {
            log::error!("Menu action not found: {key}");
            return OverlayEffect::None;
        };
        let lcfg = lessfilter_cfg();
        let mut ctx = MenuEvaluationContext::from_snapshot(snapshot, lcfg);
        if !ctx.is_applicable(action) {
            TOAST::notice(
                ToastStyle::Error,
                format!(
                    "Conditions not met for action '{}'",
                    action.display_name(key)
                ),
            );
            return OverlayEffect::None;
        }
        let targets = ctx.resolve_targets(action);
        let displays: Vec<Span<'static>> = targets.iter().map(|p| short_display(p)).collect();

        if GLOBAL::cfg().interface.bump_menu_action_paths {
            for path in &targets {
                db().bump_path(path.is_dir(), path.clone());
            }
        }

        match action.strategy {
            MenuStrategy::Queue => {
                QUEUE::enqueue(key.to_string(), targets);
                TOAST::push(ToastStyle::Normal, "Queued: ", displays);
            }
            MenuStrategy::Enqueue => {
                if let Some(index) = QUEUE::enqueue_one(key.to_string(), targets) {
                    QUEUE::dispatch(vec![index], ctx.snapshot.nav_cwd.clone());
                    TOAST::push(ToastStyle::Normal, "Started: ", displays);
                }
            }
            MenuStrategy::QueueBatch(n) => {
                // a zero batch size parses but chunks of size 0 are invalid
                let n = n.max(1);
                for chunk in targets.chunks(n) {
                    QUEUE::enqueue(key.to_string(), chunk.to_vec());
                }
                TOAST::push(ToastStyle::Normal, "Queued: ", displays);
            }
            MenuStrategy::Exec => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuAction(key.to_string()));
            }
            MenuStrategy::ExecWait => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuActionWait(key.to_string()));
            }
            MenuStrategy::ExecSilent => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuActionSilent(action.command.clone()));
            }
            MenuStrategy::ExecPaged => {
                STORE::set(MenuCommandPaths::new_from(targets));
                GLOBAL::send_action(FsAction::MenuActionExecPaged(key.to_string()));
            }
        }

        if action.closes() {
            OverlayEffect::Disable
        } else {
            OverlayEffect::None
        }
    }

    pub(super) fn on_prompt_accept(
        &mut self,
        active: ActivePrompt,
        print_flavor: bool,
    ) -> OverlayEffect {
        match active.kind {
            PromptKind::New => {
                // Relative paths resolve against the current item's parent (same as NewFolder and Extract).
                let current_item_parent = active.paths.parent;
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
                let current_item_parent = active.paths.parent;
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
                let old_path = active.paths.target;
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
                let source = active.paths.target;
                // Relative destinations resolve against the original parent directory, same as New/NewFolder.
                let dest = AbsPath::new_unchecked(Path::new(&input).abs(active.paths.parent));
                let label = short_display(&source);

                TASKS::spawn("extract", async move {
                    if let Err(e) = crate::unzip::extract_to(&source, dest.as_path()) {
                        log::error!("Failed to extract {}: {e}", source.to_string_lossy());
                        TOAST::push(ToastStyle::Error, "Failed to extract: ", [label]);
                    }
                });
            }
            PromptKind::SetAlias => {
                let path = active.paths.target;
                let alias = self.prompt.input.value();

                // in a stash pane this edits the entry's tail column instead
                let stash_name = STACK::with_current(|p| match p {
                    FsPane::Stash { stash_name, .. } => Some(stash_name.clone()),
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

        self.active_prompt = None;
        self.prompt.on_disable();
        OverlayEffect::Disable
    }
}
