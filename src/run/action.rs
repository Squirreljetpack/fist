//! note: Action handler.
//! State is managed externally: see [`super::global`] and [`super::thread_local`]

use std::path::PathBuf;

use cba::{
    bait::ResultExt, bath::PathExt, bring::split::join_with_single_quotes, broc::shell_quote,
    unwrap, wbog,
};
use matchmaker::{
    acs,
    action::{Action, Actions},
    message::Interrupt,
    nucleo::{Color, Modifier, Span, Style},
};
use ratatui::text::{Line, Text};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::{__home, text_renderer_path},
    clipboard::{copy_files, copy_paths_as_text},
    lessfilter::Preset,
    run::{
        ahandlers::{enter_dir_pane, enter_prompt, fs_reload, lock_prompt, refresh_prompt},
        dhandlers::{COPY_COMMAND, ExecutionMode},
        item::short_display,
        pane::FsPane,
        stash::STASH,
        state::{
            AcceptFlavor, ExecuteHandlerShouldProcessParent, FILTERS, GLOBAL, HideMetadata,
            InPrompt, MenuPrompt, STACK, STORE, TASKS, TOAST, context::ActionContext, sort,
        },
    },
    spawn::open_wrapped,
    ui::{
        confirm_overlay::ConfirmPrompt,
        menu_overlay::{MenuTarget, PromptKind},
    },
    utils::{text::ToastStyle, trash::trash},
};
use fist_types::When;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
/// See [crate::run::mm_config::default_binds] for the default associated keybinds.
pub enum FsAction {
    // Nav
    // ----------------------------------
    /// Enter a file or directory.
    Advance,
    /// In [FsPane::Nav], enter the parent of the current directory.
    /// In other panes, enter the parent of the current item.
    Parent,
    /// Search in the current directory.
    Find,
    /// Full text search in the current directory.
    Search,
    /// Search your visited directories/files.
    History,
    /// Jump to a directory.
    /// Relative paths are resolved relative to the home directory.
    ///
    /// # Note
    /// The char is emitted instead of jumping if the index is in the prompt.
    Jump(Vec<PathBuf>),
    /// Enter app launching pane.
    App,

    /// Go back
    Undo,
    /// Go forward
    Redo,

    // Display
    // ----------------------------------
    /// Display current filters.
    ShowFilters,
    /// Display the current stack.
    ShowStash,
    /// Clear the stack.
    ClearStash,

    CycleStash(bool),
    SwitchStash(String),
    /// Add the selection (or cwd) to the named stash and switch to its pane.
    /// Empty name = the unnamed stash.
    Stash(String),
    /// Add the selection (or cwd) to the named stash without switching panes.
    AddStash(String),
    ShowScratch,

    /// Show available actions on the current item(s).
    ShowMenu,
    /// Toggle directory/file visibility.
    /// In [`FsPane::Files`], [`FsPane::Folders`], [`FsPane::Launch`], [`FsPane::Rg`], this toggles their sort order.
    FsToggle,
    /// Toggle visibility between default and with hidden.
    ToggleHidden,

    // file actions
    // ----------------------------------
    /// Cut file (to the [`STASH`] and the system clipboard).
    Cut,
    /// Copy file (to the [`STASH`] and the system clipboard).
    Copy,
    /// Save a file to the [`STASH`] under the custom type.
    Push,
    /// Copy full path.
    CopyPath,
    /// Create a new file. Paths are relative to the current item's parent.
    New,
    /// Create a new directory. Paths are relative to the current item's parent.
    // todo: lowpri: can also add a config option to compute relative to cwd
    NewDir,
    /// Set an alias for a file or directory.
    SetAlias(String),
    /// Rename a file or directory.
    Rename,

    /// Save the file to the backup directory. (todo)
    Backup,
    /// Delete the file using system trash.
    Trash(bool),
    /// Permanently delete the file.
    Delete(bool),
    /// Internal confirmation action.
    Confirm,
    /// Paste all stack items into the current or specified directory
    Paste(PathBuf), // dump Stack
    /// Execute an action on the current item according [Lessfilter rules](crate::lessfilter::RulesConfig)
    Lessfilter {
        preset: Preset,
        paging: bool, // whether to feed the output to a pager
        header: When,
        special: u8,
    },
    /// Preview a file using a preset
    LessfilterPreview(Preset, When),
    // Execute
    // ----------------------------------
    /// Execute a command template formatted against the current item (or the
    /// cwd while the cursor is disabled), waiting for it to exit.
    Execute(String),
    /// Like [`Self::Execute`], but pipe stdout through the pager.
    ExecPaged(String),
    /// Like [`Self::Execute`], but connect stdin/stdout/stderr to the tty.
    ExecTTY(String),
    /// Execute silently (null stdio, no wait) and detached (`.detach()`).
    ExecDetached(String),
    /// Execute silently (null stdio, no wait) without detaching.
    ExecSilent(String),
    /// Execute a command synchronously and copy its stdout to the clipboard.
    CopyCommand(String),
    /// Execute a command in the background and copy its stdout to the clipboard.
    CopyCommandAsync(String),
    // Nonbindable
    // ----------------------------------
    SaveInput,
    SetHeader(Option<Text<'static>>),
    SetFooter(Option<Text<'static>>),
    Reload,
    /// Apply the current pane sort to the worker (also lands pending fill values).
    ReSort,
    /// Sync visibility pane ← global, then reload (db/rg/vis change) or fill+resort (sort change).
    Refilter,
    AcceptPrompt,
    Filtering(Option<bool>),
    SetStatus(Option<Line<'static>>),

    // Other
    // ----------------------------------
    /// Enter (Some(true)) / leave (Some(false)) the prompt; None toggles.
    LockPrompt(Option<bool>),
    /// Jump and accept;
    /// 0 jumps to menu.
    AutoJump(u8),
}
// print, accept

impl FsAction {
    // #[inline]
    // fn unchecked_jump(p: AbsPath) -> Self {
    //     Self::Jump(p.into(), Some('\0'))
    // }
    #[inline]
    pub fn set_footer(p: impl Into<Option<Text<'static>>>) -> Self {
        Self::SetFooter(p.into())
    }
    #[inline]
    pub fn set_header(p: impl Into<Option<Text<'static>>>) -> Self {
        Self::SetHeader(p.into())
    }

    pub fn new_lessfilter(
        preset: Preset,
        paging: bool,
    ) -> Self {
        Self::Lessfilter {
            preset,
            paging,
            header: When::Auto,
            special: Default::default(),
        }
    }

    pub fn help() -> Self {
        Self::Lessfilter {
            preset: Preset::Preview,
            paging: true,
            header: When::Auto,
            special: 1,
        }
    }
}

// --------- HELPERS ------------

// -------------------- ALIASER ------------------------------------

// note: since this happens before the batch process of actions, we do not support chaining custom actions
// i.e. "current" saved inputs in chained actions, or consecutive nav actions

// todo: get rid of aliaser for effects
pub fn fsaction_aliaser(
    a: Action<FsAction>,
    state: &mut MMState<'_, '_>,
) -> Actions<FsAction> {
    // prompt-mode state: the raw InPrompt marker (the query bar is active).
    // With prompt_locking on, the direct pathways (LockPrompt action, pane
    // lock_prompt config, --lock-prompt) set it; with locking off they are
    // no-ops, so only the cwd lock (enter_prompt) does.
    let in_prompt = STORE::contains::<InPrompt>();
    let raw_input = in_prompt || state.overlay_index().is_some();

    match a {
        Action::Custom(fa) => match fa {
            // handle nonbindable events here so that overlays don't intercept them.
            // -------------------------------------------------
            FsAction::Reload => {
                fs_reload(state, false, false);
                acs![]
            }
            // user sets sort => ::Refilter => set_sort_from_pane => fill_and_resort => ::ReSort => set_sort_from_pane
            // problem: we set global sort to tell what to inject, but rendered sizes may be stale, this is not a big problem as this only
            // conflates mtime/atime, the other sort modes don't depend on the Atomic metadata field.
            FsAction::ReSort => {
                // sort explicitly set: unhide the metadata column
                STORE::take::<HideMetadata>();
                sort::set_sort_from_pane(state);
                state.worker_resort();
                state.picker_ui.results.set_dirty();
                acs![]
            }
            FsAction::Refilter => {
                // 1. sync visibility pane <- global
                let vis_changed = STACK::with_current_mut(|p| {
                    p.vis_mut().is_some_and(|v| {
                        let vis = FILTERS::visibility();
                        let changed = *v != vis;
                        *v = vis;
                        changed
                    })
                });
                if vis_changed {
                    // populate re-reads pane sort/vis; fs_reload -> set_sort_in_nucleo applies the mode
                    fs_reload(state, false, false);
                } else {
                    // db/rg panes
                    // order via SQL/rg, so a reload is required; everything
                    // else fills metadata and dispatches ReSort.
                    if STACK::reloads_by_sorting() {
                        fs_reload(state, false, false);
                    } else {
                        sort::fill_then_resort(state);
                    }
                }
                acs![]
            }
            FsAction::SaveInput => {
                let (content, index) = (
                    state.picker_ui.query.input(),
                    state.picker_ui.results.index(),
                );
                log::debug!("Saved: {content}, {index}");
                STACK::save_input(content, index);

                acs![]
            }
            FsAction::SetHeader(text) => {
                if let Some(text) = text {
                    state.picker_ui.header.set(text);
                } else {
                    state.picker_ui.header.clear(true);
                }
                acs![]
            }
            FsAction::SetFooter(text) => {
                if let Some(text) = text {
                    state.footer_ui.set(text);
                } else {
                    state.footer_ui.clear(false);
                }
                acs![]
            }
            FsAction::Filtering(s) => {
                if let Some(s) = s {
                    state.picker_ui.filtering = s
                } else {
                    state.picker_ui.filtering = !state.picker_ui.filtering
                };
                acs![]
            }
            FsAction::SetStatus(s) => {
                state.picker_ui.status.set(s);
                acs![]
            }

            // Actions which only trigger when not in the prompt:
            // -------------------------------------------------
            FsAction::Parent => {
                if raw_input {
                    acs![Action::BackwardChar]
                } else if STACK::in_app() {
                    // todo()
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }
            FsAction::Advance => {
                if raw_input {
                    acs![Action::ForwardChar]
                } else if STACK::in_app() {
                    // todo!()
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }
            FsAction::Delete(no_confirm) => {
                // lowpri: maybe should require a global_config
                // edit-actions edit the query while the prompt mode is on,
                // regardless of prompt_locking (raw marker, not in_prompt)
                if STORE::contains::<InPrompt>() && !no_confirm {
                    acs![Action::DeleteWord]
                } else if STACK::in_app() {
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }
            FsAction::Trash(no_confirm) => {
                if in_prompt && !no_confirm {
                    acs![Action::DeleteWord]
                } else if STACK::in_app() {
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }

            //  ------------- Overlay aliases --------------
            FsAction::ShowStash
            | FsAction::ShowScratch
            | FsAction::ShowFilters
            | FsAction::Confirm
            | FsAction::ShowMenu
                if state.overlay_index().is_some() =>
            {
                acs![fa]
            }
            FsAction::ShowStash => {
                acs![Action::Overlay(0)]
            }
            FsAction::ShowScratch => {
                acs![Action::Overlay(1)]
            }
            FsAction::ShowFilters => {
                acs![Action::Overlay(2)]
            }
            FsAction::Confirm => {
                acs![Action::Overlay(3)]
            }
            // todo: matchmaker needs to support activating the overlay ourselves so that the activated item is aligned
            FsAction::ShowMenu => {
                if let Some((_, p)) = state.picker_ui.current_indexed() {
                    STORE::set_menu_target(MenuTarget::Item(p.path.clone()));
                    acs![Action::Overlay(4)]
                } else if let Some(cwd) = STACK::cwd() {
                    STORE::set_menu_target(MenuTarget::Item(cwd.clone()));
                    acs![Action::Overlay(4)]
                } else {
                    acs![]
                }
            }
            // todo: support post-creation actions
            FsAction::New => {
                if state.overlay_index().is_some() {
                    return acs![];
                }
                // no support for creating outside of nav
                if state.current_raw().is_some() || STACK::nav_cwd().is_some() {
                    STORE::set_menu_prompt(Some(MenuPrompt::new(PromptKind::New)));
                    acs![Action::Overlay(4)]
                } else {
                    acs![]
                }
            }
            FsAction::NewDir => {
                if state.overlay_index().is_some() {
                    return acs![];
                }
                // no support for creating outside of nav
                if state.current_raw().is_some() || STACK::nav_cwd().is_some() {
                    STORE::set_menu_prompt(Some(MenuPrompt::new(PromptKind::NewDir)));
                    acs![Action::Overlay(4)]
                } else {
                    acs![]
                }
            }
            FsAction::SetAlias(_) => {
                if in_prompt || STACK::in_rg() || state.overlay_index().is_some() {
                    return acs![];
                }
                if let Some(item) = state.current_raw() {
                    let prepop_value = item.tail_text().to_string();
                    STORE::set_menu_prompt(Some(
                        MenuPrompt::new(PromptKind::SetAlias).initial(prepop_value),
                    ));
                    STORE::set_menu_target(MenuTarget::Item(item.path.clone()));
                    acs![Action::Overlay(4)]
                } else {
                    acs![]
                }
            }
            FsAction::LessfilterPreview(preset, header) => {
                acs![Action::Preview(preset.to_command_string(header))]
            }
            // FsAction::Category => {
            //     acs![Action::Overlay(3)]
            // }
            FsAction::AutoJump(digit) => {
                if state.overlay_index().is_some()
                // in overlay
                {
                    acs![Action::Pos(digit.saturating_sub(1) as i32)]
                } else if digit == 0
                // 0 -> TogglePrompt
                {
                    if in_prompt {
                        lock_prompt(state, false);
                    } else {
                        enter_prompt(state);
                    }
                    acs![]
                } else if in_prompt
                // in prompt => jump out
                {
                    lock_prompt(state, false);
                    acs![Action::Pos(digit as i32 - 1)]
                } else if (digit - 1) as u32 == state.picker_ui.results.index()
                // not in prompt + on pos => accept
                {
                    let accept: Action<FsAction> =
                        if GLOBAL::with_cfg(|c| c.interface.autojump_advance) {
                            FsAction::Advance.into()
                        } else {
                            if GLOBAL::with_cfg(|c| c.interface.alt_accept) && !STACK::in_app() {
                                STORE::set(AcceptFlavor);
                            }
                            Action::Accept
                        };
                    acs![Action::Pos((digit - 1) as i32), accept]
                } else {
                    acs![Action::Pos((digit - 1) as i32),]
                }
            }
            _ => acs![fa],
        },
        _ => match a {
            // these can technically be more
            Action::Up(i) => {
                TOAST::clear();

                if state.overlay_index().is_some() {
                    acs![a]
                } else if state.picker_ui.results.cursor_disabled() {
                    // locked onto the cwd: leave the prompt (unlocks) and move
                    lock_prompt(state, false);
                    if !state.picker_ui.reverse() {
                        acs![a]
                    } else {
                        acs![Action::Up(i.saturating_sub(1))]
                    }
                } else if i as u32 > state.picker_ui.results.index() && !state.picker_ui.reverse() {
                    // going up past the first item: enter the prompt and lock
                    // the active item onto the cwd. Apps panes never enter
                    // the prompt — the Up then passes through untouched.
                    if enter_prompt(state) { acs![] } else { acs![a] }
                } else {
                    acs![a]
                }
            }
            Action::Down(i) => {
                TOAST::clear();

                if state.overlay_index().is_some() {
                    acs![a]
                } else if state.picker_ui.results.cursor_disabled() {
                    // locked onto the cwd: leave the prompt (unlocks) and move
                    lock_prompt(state, false);
                    if state.picker_ui.reverse() {
                        acs![a]
                    } else {
                        acs![Action::Down(i.saturating_sub(1))]
                    }
                } else if i as u32 > state.picker_ui.results.index() && state.picker_ui.reverse() {
                    // mirror of Up: past the last item in reverse mode enters
                    // the prompt and locks onto the cwd (Apps panes pass the
                    // Down through untouched)
                    if enter_prompt(state) { acs![] } else { acs![a] }
                } else {
                    acs![a]
                }
            }

            // Accept (enter) and Print("") (alt-enter) share one arm: the
            // print-vs-open decision lives in the make_mm accept hook, which
            // reads the AcceptFlavor flag set here. Non-empty Print payloads
            // pass through to the interrupt handler untouched.
            Action::Accept | Action::Print(_) => {
                // non-empty Print payloads pass through to the interrupt handler;
                // overlays own accept keys while open
                if matches!(a, Action::Print(ref s) if !s.is_empty())
                    || state.overlay_index().is_some()
                {
                    acs![a]
                } else if in_prompt {
                    if state.picker_ui.results.cursor_disabled() {
                        // already locked: accept on the cwd
                        acs![FsAction::AcceptPrompt]
                    } else if enter_prompt(state) {
                        // first accept in the prompt: lock onto the cwd, swallow
                        acs![]
                    } else {
                        acs![a]
                    }
                } else {
                    // alt_accept swaps the two flavors (XOR); apps always open
                    let is_print = matches!(a, Action::Print(_));
                    let print_flavor = (GLOBAL::with_cfg(|c| c.interface.alt_accept) ^ is_print)
                        && !STACK::in_app();
                    if print_flavor {
                        STORE::set(AcceptFlavor);
                    }
                    acs![Action::Accept]
                }
            }

            Action::Reload(s)
                if s.is_empty()
                    && STACK::with_current(|c| matches!(c, FsPane::Custom { cmd: None, .. })) =>
            {
                TOAST::msg("Cannot reload streams", false);
                acs![]
            }

            Action::ForwardChar | Action::BackwardChar
                if !state.picker_ui.results.cursor_disabled() && STORE::contains::<InPrompt>() =>
            {
                acs![if matches!(a, Action::ForwardChar) {
                    FsAction::Advance
                } else {
                    FsAction::Parent
                }]
            }

            _ => acs![a],
        },
    }
}

pub fn fsaction_handler(
    a: FsAction,
    state: &mut MMState<'_, '_>,
    context: &mut ActionContext,
) {
    let print_handle = &context.print_handle;

    match a {
        FsAction::LockPrompt(enter) => {
            lock_prompt(state, enter.unwrap_or(!STORE::contains::<InPrompt>()));
        }
        FsAction::Find => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // pane
            let pane = FsPane::new_fd(STACK::_cwd(), sort::get_sort().order, FILTERS::visibility());

            // don't push if same pane: changes in filter/vis already should be the ones to responsible for that (todo?)
            // todo: there is a problem
            if STACK::with_current(|p| *p == pane) {
                fs_reload(state, false, false);
            } else {
                STACK::push(pane);
                fs_reload(state, true, false);
            }

            // not this because this erases current settings when the intutive behavior is to just reload
            // if STACK::set_or_push(pane) {
            //     prepare_prompt(state);
            //     fs_reload(state, true);
            // } else {
            //     fs_reload(state, false);
            // }
        }

        FsAction::History => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            let _ = STACK::swap_history();

            fs_reload(state, true, false);
        }

        FsAction::Search => {
            if STACK::with_current_mut(|x| match x {
                FsPane::Search {
                    filtering,
                    patterns,
                    input,
                    is_initial,
                    ..
                } => {
                    *filtering = !*filtering;

                    // update state -> UI
                    let new_input = if *filtering {
                        // entering filter:
                        // restore from input
                        input.0.clone()
                    } else {
                        // entering rg:

                        // save query
                        input.0 = state.picker_ui.query.input();
                        // set picker.input to previous patterns
                        join_with_single_quotes(patterns)
                    };
                    state.picker_ui.query.set(new_input, u16::MAX);

                    // the hook updates UI from state when is_new and vv. Here we don't want to update other UI config parts, but we do want to update UI -> state, so we repeat that here and set initial to true as a marker telling the reload hook to skip state -> UI. Technically this marker is not necessary but it's more logical.
                    *is_initial.borrow_mut() = true;

                    true
                }
                _ => false,
            }) {
                fs_reload(state, false, false);
            } else {
                // save input
                let (content, index) = state.get_content_and_index();
                STACK::save_input(content, index);

                let [one_line, fixed_strings] =
                    GLOBAL::with_cfg(|c| [c.panes.search.one_line, c.panes.search.fixed_strings]);

                let cwd = STACK::_cwd();

                let paths = if state.selections().is_empty() {
                    vec![]
                } else {
                    state.map_selected_to_vec(|_i, x| x.path.inner())
                };

                let query = String::new();
                let filtering = false;

                let context = Default::default();
                let case = Default::default();

                let pane = FsPane::new_rg(
                    cwd,
                    sort::get_sort().order,
                    FILTERS::visibility(),
                    //
                    paths,
                    query,
                    vec![],
                    filtering,
                    //
                    context,
                    case,
                    one_line,
                    fixed_strings,
                    vec![],
                );
                STACK::push(pane);
                fs_reload(state, true, false);
            }
        }

        FsAction::App => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            STORE::set(STASH::current_scratch());

            let pane = FsPane::new_launch();
            if STACK::set_or_push(pane) {
                fs_reload(state, true, false);
            } else {
                fs_reload(state, false, false);
            }
        }

        FsAction::Undo => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // adjust stack
            if STACK::stack_prev() {
                fs_reload(state, true, true);
            };
        }
        FsAction::Redo => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // adjust stack
            if STACK::stack_next() {
                fs_reload(state, true, true);
            };
        }

        FsAction::Jump(mut paths) => {
            let cwd = STACK::cwd().and_then(|p| p.canonicalize().ok());

            // jump between home and current root if empty (seems reasonable)
            if paths.is_empty() {
                paths = vec![
                    __home().into(),
                    cba::bath::find_root().unwrap_or(PathBuf::from(std::path::MAIN_SEPARATOR_STR)),
                ];
            }

            let canonical = |p: &std::path::Path| p.abs(__home()).canonicalize().ok();

            let idx = if cwd.is_some() {
                paths
                    .iter()
                    .position(|p| canonical(p) == cwd)
                    .map(|i| (i + 1) % paths.len())
                    .unwrap_or(0)
            } else {
                0
            };

            let target_path = paths[idx].abs(__home());

            if target_path.is_dir() {
                let abs_target = AbsPath::new_unchecked(target_path);
                if Some(&abs_target) != STACK::cwd().as_ref() {
                    enter_dir_pane(state, abs_target);
                }
            } else {
                TOAST::msg(
                    vec![
                        Span::styled(target_path.to_string_lossy().to_string(), Color::Red),
                        Span::raw(" is not a valid directory!"),
                    ],
                    false,
                );
            }
        }
        FsAction::Parent => {
            // If Nav, go to the parent of the cwd, otherwise go to the parent of the current item

            let current = if let Some(p) = STACK::nav_cwd() {
                p
            } else {
                unwrap!(state.current_raw().map(|x| x.path.clone()))
            };
            let path = unwrap!(current.parent().map(AbsPath::new_unchecked));

            // save current for lookup
            STORE::set(current);
            // pane
            enter_dir_pane(state, path);
        }

        FsAction::Advance => {
            let Some(item) = &state.current_raw() else {
                return;
            };

            if item.path.is_dir() {
                // pane
                enter_dir_pane(state, item.path.clone())
            } else if item.path.exists() {
                // record
                if item.path.is_file() {
                    GLOBAL::db().bump_path(false, item.path.clone());
                }

                // todo: specialized
                let template = GLOBAL::with_cfg(|c| c.interface.advance_command.clone());
                state.set_interrupt(Interrupt::Execute, template);
            }
        }

        // File actions
        // --------------------------------
        // the active item is the cwd while cursor_disabled (see Push)
        FsAction::Cut => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            let items = if state.picker_ui.results.cursor_disabled() {
                STACK::cwd()
                    .into_iter()
                    .inspect(|p| {
                        toast_vec.push(short_display(p));
                        cb_vec.push(p.inner());
                    })
                    .collect::<Vec<_>>()
            } else {
                state.map_selected_to_vec(|_i, s| {
                    toast_vec.push(short_display(&s.path));
                    cb_vec.push(s.path.inner());
                    s.path.clone()
                })
            };
            if !items.is_empty() {
                STASH::extend("cut", items);
                TOAST::push(ToastStyle::Normal, "Cut: ", toast_vec);
                copy_files(cb_vec, false);
            };
        }
        FsAction::Copy => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            let items = if state.picker_ui.results.cursor_disabled() {
                STACK::cwd()
                    .into_iter()
                    .inspect(|p| {
                        toast_vec.push(short_display(p));
                        cb_vec.push(p.inner());
                    })
                    .collect::<Vec<_>>()
            } else {
                state.map_selected_to_vec(|_, s| {
                    toast_vec.push(short_display(&s.path));
                    cb_vec.push(s.path.inner());
                    s.path.clone()
                })
            };
            if !items.is_empty() {
                STASH::extend("copy", items);
                TOAST::push(ToastStyle::Normal, "Copied: ", toast_vec);
                copy_files(cb_vec, false);
            };
        }

        // Note: This is the only stash action which also pushes the cwd
        FsAction::Push => {
            let mut toast_vec = vec![];

            if !state.picker_ui.results.cursor_disabled() {
                let items = state.map_selected_to_vec(|_, s| {
                    toast_vec.push(short_display(&s.path));
                    s.path.clone()
                });
                if !items.is_empty() {
                    STASH::extend("copy", items);
                }
            } else if let Some(p) = STACK::cwd() {
                toast_vec.push(short_display(&p));
                STASH::stash("copy", p);
            };

            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Stashed: ", toast_vec);
            };
        }

        // Named stash actions: add the selection (or cwd) to the `stashes`
        // db table; `Stash` additionally switches to the stash pane. The
        // db task triggers a reload once the inserts complete.
        fa @ (FsAction::Stash(_) | FsAction::AddStash(_)) => {
            let (name, switch) = match fa {
                FsAction::Stash(n) => (n, true),
                FsAction::AddStash(n) => (n, false),
                _ => unreachable!(),
            };

            let mut toast_vec = vec![];
            let items = if state.picker_ui.results.cursor_disabled() {
                STACK::cwd()
                    .into_iter()
                    .inspect(|p| toast_vec.push(short_display(p)))
                    .collect::<Vec<_>>()
            } else {
                state.map_selected_to_vec(|_, s| {
                    toast_vec.push(short_display(&s.path));
                    s.path.clone()
                })
            };
            if items.is_empty() {
                return;
            }

            if switch {
                let (content, index) = state.get_content_and_index();
                STACK::save_input(content, index);
                // a same-variant current pane is replaced in place, which
                // also covers switching between stash names
                STACK::set_or_push(FsPane::new_stash(name.clone(), sort::get_sort().order));
            }

            db_stash(name.clone(), items);

            if switch {
                fs_reload(state, true, false);
            }

            let mut line = Line::from(vec![Span::styled(
                format!("Stashed ({}): ", name),
                ToastStyle::Normal,
            )]);
            line.spans.extend(toast_vec);
            TOAST::msg(line, false);
        }

        FsAction::CycleStash(forwards) => {
            STASH::cycle_scratch(forwards);
            TOAST::notice(
                ToastStyle::Normal,
                format!("Scratch: {}", STASH::current_scratch()),
            );
        }
        FsAction::SwitchStash(s) => {
            if STASH::set_scratch(&s) {
                TOAST::notice(ToastStyle::Info, format!("Scratch: {s}"));
            } else {
                TOAST::notice(ToastStyle::Error, format!("No such Scratch: {s}"));
            }
        }

        FsAction::Backup => {
            // todo: impl using custom stash + some kind of db-based kv store
        }

        FsAction::Trash(no_confirm) => {
            // in a stash pane, Trash removes from the stash, not the actual path
            let stash_name = STACK::with_current(|p| match p {
                FsPane::Stash { stash_name, .. } => Some(stash_name.clone()),
                _ => None,
            });

            let mut items = vec![];
            state.map_selected_to_vec(|_, s| {
                items.push(s.path.inner());
            });

            if items.is_empty() {
                return;
            }

            if !no_confirm {
                let prompt = if items.len() == 1 {
                    Line::from_iter([
                        Span::styled("Trash", Color::Red),
                        Span::raw(format!(
                            " {}?",
                            short_display(&AbsPath::new_unchecked(&items[0]))
                        )),
                    ])
                } else {
                    Line::from_iter([
                        Span::styled("Trash", Color::Red),
                        Span::raw(format!(" {} items?", items.len())),
                    ])
                };

                STORE::set(ConfirmPrompt {
                    prompt,
                    options: vec![("Yes", 0), ("No", 0)],
                    option_handler: Box::new(|idx| {
                        if idx == 0 {
                            GLOBAL::send_action(FsAction::Trash(true));
                        }
                    }),
                    content: None,
                    content_above: false,
                    title_in_border: false,
                    cursor: 0, // Default to Yes
                    scroll: 0,
                });
                GLOBAL::send_action(FsAction::Confirm);
                return;
            }

            if let Some(name) = stash_name {
                db_stash_remove(name, items);
                return;
            }

            // not heavy computationally, but still blocking...
            TASKS::spawn_blocking(|| {
                for path in items {
                    match trash(&path) {
                        Ok(()) => {
                            TOAST::push(ToastStyle::Success, "Trashed: ", [short_display(&path)]);
                        }
                        Err(e) => {
                            log::error!("Failed to trash {}: {e}", path.to_string_lossy());
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to trash: ",
                                [short_display(&path)],
                            );
                        }
                    }
                }
            });
        }
        FsAction::Delete(no_confirm) => {
            // in a stash pane, Delete removes from the stash, not the actual path
            let stash_name = STACK::with_current(|p| match p {
                FsPane::Stash { stash_name, .. } => Some(stash_name.clone()),
                _ => None,
            });

            let mut items = vec![];
            state.map_selected_to_vec(|_, s| {
                items.push(s.path.inner());
            });

            if items.is_empty() {
                return;
            }

            if !no_confirm {
                let prompt = if items.len() == 1 {
                    Line::from_iter([
                        Span::styled("Delete", Color::Red),
                        Span::raw(format!(
                            " {}?",
                            short_display(&AbsPath::new_unchecked(&items[0]))
                        )),
                    ])
                } else {
                    Line::from_iter([
                        Span::styled("Delete", Color::Red),
                        Span::raw(format!(" {} items?", items.len())),
                    ])
                };

                STORE::set(ConfirmPrompt {
                    prompt,
                    options: vec![("Yes", 0), ("No", 0)],
                    option_handler: Box::new(|idx| {
                        if idx == 0 {
                            GLOBAL::send_action(FsAction::Delete(true));
                        }
                    }),
                    content: None,
                    content_above: false,
                    title_in_border: false,
                    cursor: 1, // Default to No
                    scroll: 0,
                });
                GLOBAL::send_action(FsAction::Confirm);
                return;
            }

            if let Some(name) = stash_name {
                db_stash_remove(name, items);
                return;
            }

            TASKS::spawn(async move {
                for path in items {
                    let result = if path.is_dir() {
                        tokio::fs::remove_dir_all(&path).await
                    } else {
                        tokio::fs::remove_file(&path).await
                    };

                    match result {
                        Ok(()) => {
                            TOAST::push(ToastStyle::Success, "Deleted: ", [short_display(&path)]);
                        }
                        Err(e) => {
                            log::error!("Failed to delete {}: {e}", path.to_string_lossy());
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to delete: ",
                                [short_display(&path)],
                            );
                        }
                    }
                }
            });
        }
        FsAction::Confirm => {}
        FsAction::CopyPath => {
            let paths = if !state.picker_ui.results.cursor_disabled() {
                state.map_selected_to_vec(|_, s| s.path.inner())
            } else {
                STACK::cwd().map(PathBuf::from).into_iter().collect()
            };

            copy_paths_as_text(paths, true);
        }
        FsAction::Paste(dest_base) => {
            let base = if dest_base.is_empty() {
                if let Some(c) = STACK::nav_cwd() {
                    c
                } else {
                    TOAST::notice(ToastStyle::Normal, "No current directory.");
                    return;
                }
            } else {
                if !dest_base.is_absolute() {
                    TOAST::notice(
                        ToastStyle::Error,
                        format!("{} is not absolute.", dest_base.to_string_lossy()),
                    );
                    return;
                }
                AbsPath::new_unchecked(dest_base)
            };
            STASH::execute_all_impl(base, false, None);
        }
        FsAction::ClearStash => {
            // STASH::clear(x.as_deref());

            TOAST::notice(ToastStyle::Normal, "Stack cleared");
        }
        // filters
        FsAction::FsToggle => {
            if STACK::with_current(|p| matches!(p, FsPane::Files { .. } | FsPane::Folders { .. })) {
                STACK::with_current_mut(|p| p.sort_mut().cycle());
            } else {
                FILTERS::with_mut(|vis| {
                    (vis.dirs, vis.files) = match (vis.dirs, vis.files) {
                        (false, false) => (false, true),
                        (false, true) => (true, false),
                        (true, false) => (false, false),
                        (true, true) => {
                            log::error!("Unexpected toggle dirs state");
                            (false, false)
                        }
                    };
                });
            }
            // anyone who modifies pane sort/vis must immediately follow with Refilter
            GLOBAL::send_action(FsAction::Refilter);
            refresh_prompt(state);
        }
        FsAction::ToggleHidden => {
            FILTERS::with_mut(|vis| {
                let style = Style::new().add_modifier(Modifier::DIM).italic();
                if vis.hidden || vis.all() {
                    vis.set_default();
                    TOAST::msg(Span::styled("Default filters", style), true);
                } else {
                    vis.hidden = true;
                    TOAST::msg(Span::styled("Showing hidden", style), true);
                }
            });
            GLOBAL::send_action(FsAction::Refilter);
        }
        // ------------------------------------------------------
        // Execute/Accept
        FsAction::Lessfilter {
            preset,
            paging,
            header,
            special,
        } => {
            if STACK::in_app() {
                // todo
                return;
            }

            if state.current_raw().is_none() && !state.picker_ui.results.cursor_disabled() {
                return;
            };

            // since in Nav pane, Advance is bound to edit cursor item, it's more useful to make the action always edit the menu item.
            if matches!(preset, Preset::Edit)
                && state.current_raw().is_some_and(|x| x.path.is_file())
            {
                STORE::set(ExecuteHandlerShouldProcessParent {});
            }

            let mut template = if special == 1 {
                format!(
                    "'{}' :tool show-binds",
                    crate::cli::paths::current_exe()
                        .to_str()
                        .unwrap_or(crate::cli::paths::BINARY_SHORT),
                )
            } else {
                preset.to_command_string(header)
            };

            if paging {
                // we need to use the renderer because the first pass of renderer won't render when it sees it is being piped
                if let Some(pp) = shell_quote(text_renderer_path()) {
                    // Match the special code to its corresponding environment variable setting
                    let env_var = match special {
                        1 => Some("PG_LANG=ini"),
                        _ => None,
                    };

                    #[cfg(windows)]
                    {
                        if let Some(env) = env_var {
                            template.push_str(&format!(" | cmd /c \"set {env} && {pp}\" > CON"));
                        } else {
                            template.push_str(&format!(" | cmd /c \"{pp}\" > CON"));
                        }
                    }

                    #[cfg(unix)]
                    {
                        if let Some(env) = env_var {
                            template.push_str(&format!(" | {env} {pp} > /dev/tty"));
                        } else {
                            template.push_str(&format!(" | {pp} > /dev/tty"));
                        }
                    }
                } else {
                    wbog!(
                        "Pager path could not be decoded, please check your installation's cache directory."
                    )
                }
            }

            state.set_interrupt(Interrupt::Execute, template);
        }

        // See the handlers in [`crate::run::dhandlers`]: the mode is
        // transported in `state.discriminant_payload`, the template stays
        // untouched.
        fa @ (FsAction::Execute(_) | FsAction::ExecPaged(_) | FsAction::ExecTTY(_)) => {
            let (mode, template) = match fa {
                FsAction::Execute(t) => (ExecutionMode::Normal, t),
                FsAction::ExecPaged(t) => (ExecutionMode::Paged, t),
                FsAction::ExecTTY(t) => (ExecutionMode::Tty, t),
                _ => unreachable!(),
            };
            state.discriminant_payload = Some(mode.discriminant());
            state.set_interrupt(Interrupt::Execute, template);
        }
        FsAction::ExecDetached(template) => {
            state.discriminant_payload = Some(ExecutionMode::Detached.discriminant());
            state.set_interrupt(Interrupt::ExecuteSilent, template);
        }
        FsAction::ExecSilent(template) => {
            state.discriminant_payload = Some(ExecutionMode::Silent.discriminant());
            state.set_interrupt(Interrupt::ExecuteSilent, template);
        }
        FsAction::CopyCommand(template) => {
            state.discriminant_payload = Some(COPY_COMMAND);
            state.set_interrupt(Interrupt::ExecuteSilent, template);
        }
        FsAction::CopyCommandAsync(template) => {
            state.discriminant_payload = Some(COPY_COMMAND);
            state.set_interrupt(Interrupt::ExecuteAsync, template);
        }

        FsAction::AcceptPrompt => {
            if let Some(p) = STACK::nav_cwd() {
                if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                    // same as below
                    let s = p.display().to_string();
                    print_handle.push(s);

                    GLOBAL::db().bump_path(true, p);

                    state.picker_ui.selector.clear();
                    state.should_quit = true;
                } else {
                    // accepting on nav pane prompt opens the displayed directory
                    let path = p.inner().into();
                    let pool = GLOBAL::db();

                    TASKS::spawn(async move {
                        let conn = unwrap!(pool.get_conn(crate::db::DbTable::dirs).await.ok());
                        open_wrapped(conn, None, &[path], true).await._elog();
                    });

                    // this one is conditional unlike the rest
                    if state.selections().is_empty() {
                        state.should_quit = true;
                    }
                }
            } else if let Some(cwd) = STACK::cwd() {
                enter_dir_pane(state, cwd);
            }
        }

        _ => {
            log::error!("Encountered unreachable {a:?}");
            unreachable!()
        }
    }
}

/// Insert `paths` into the named stash, then reload once the inserts
/// complete ("AddStash adds it, then triggers a reload after added").
fn db_stash(
    name: String,
    paths: Vec<AbsPath>,
) {
    let pool = GLOBAL::db();
    TASKS::spawn(async move {
        match pool.get_conn(crate::db::DbTable::stashes).await {
            Ok(mut conn) => {
                for path in paths {
                    if let Err(e) = conn.add_stash_entry(&name, &path).await {
                        log::error!("Error adding stash entry: {e}");
                    }
                }
            }
            Err(e) => {
                log::error!("Error getting connection: {e}");
            }
        }
        GLOBAL::send_action(FsAction::Reload);
    });
}

/// Remove `paths` from the named stash, then reload once the removals
/// complete. Used by Trash/Delete inside a stash pane.
fn db_stash_remove(
    name: String,
    paths: Vec<PathBuf>,
) {
    let pool = GLOBAL::db();
    TASKS::spawn(async move {
        let paths: Vec<AbsPath> = paths
            .iter()
            .map(|p| AbsPath::new_unchecked(p))
            .collect();
        match pool.get_conn(crate::db::DbTable::stashes).await {
            Ok(mut conn) => match conn.remove_stash_entries(&name, &paths).await {
                Ok(n) => {
                    TOAST::notice(
                        ToastStyle::Success,
                        format!("Removed {n} item(s) from stash ({name})"),
                    );
                }
                Err(e) => {
                    log::error!("Error removing stash entries: {e}");
                    TOAST::notice(ToastStyle::Error, "Failed to remove stash entries.");
                }
            },
            Err(e) => {
                log::error!("Error getting connection: {e}");
            }
        }
        GLOBAL::send_action(FsAction::Reload);
    });
}

// ------------- BOILERPLATE ---------------
enum_from_str_display! {
    FsAction;

    units:
    Advance, Parent, Find, Search, History, App,
    Undo, Redo, Push,
    ShowFilters, ShowStash, ShowScratch,
    ShowMenu, FsToggle, ToggleHidden,
    Cut, Copy, CopyPath, New, NewDir, Rename,
    Backup, ClearStash;

    tuples:
    AutoJump, SwitchStash, SetAlias,
    ExecPaged, ExecTTY, ExecDetached, ExecSilent, CopyCommand, CopyCommandAsync;

    defaults:
    (Delete, false), (Trash, false), (Stash, String::new()), (AddStash, String::new()), (CycleStash, true)
    ;
    options:
    LockPrompt;

    lossy:
    Paste;
}

macro_rules! enum_from_str_display {
                    (
                        $enum:ty;
                        units: $( $unit:ident ),* $(,)?;
                        tuples: $( $tuple:ident ),* $(,)?;
                        defaults: $(($default:ident, $default_value:expr)),*;
                        options: $($optional:ident),*;
                        lossy: $( $lossy:ident ),* ;
                    ) => {
                        impl std::fmt::Display for $enum {
                            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                                use $enum::*;
                                match self {
                                    $( $unit => write!(f, stringify!($unit)), )*

                                    $( $tuple(inner) => write!(f, concat!(stringify!($tuple), "({})"), inner), )*

                                    $( $lossy(inner) => {
                                        if inner.is_empty() {
                                            write!(f, stringify!($lossy))
                                        } else {
                                            write!(f, concat!(stringify!($lossy), "({})"), std::ffi::OsString::from(inner).to_string_lossy())
                                        }
                                    }, )*

                                    $( $default(inner) => {
                                        if *inner == $default_value {
                                            write!(f, stringify!($default))
                                        } else {
                                            write!(f, concat!(stringify!($default), "({})"), inner)
                                        }
                                    }, )*

                                    $( $optional(opt) => {
                                        if let Some(inner) = opt {
                                            write!(f, concat!(stringify!($optional), "({})"), inner)
                                        } else {
                                            write!(f, stringify!($optional))
                                        }
                                    }, )*

                                    /* ---------- Manually parsed ---------- */
                                    Jump(paths) => {
                                        if paths.is_empty() {
                                            write!(f, "Jump(⌂⦀/)")
                                        } else {
                                            write!(f, "Jump({})", paths
                                            .iter()
                                            .map(|p| p.to_string_lossy())
                                            .collect::<Vec<_>>()
                                            .join(",")
                                        )
                                    }
                                }
                                SaveInput | SetHeader(_) | SetFooter(_) | Reload | ReSort | Refilter | AcceptPrompt | Filtering(_) | SetStatus(_) | Confirm => Ok(()), // internal
                                Lessfilter { preset, paging, header: _, special, } => {
                                    if *special == 1 {
                                        write!(f, "Help")
                                    }
                                    else if *paging {
                                        write!(f, "LFPaged({preset})")
                                    } else {
                                        write!(f, "Lessfilter({preset})")
                                    }

                                },
                                Execute(s) => write!(f, "Execute({})", s),

                                LessfilterPreview(p, _) => {
                                    write!(f, "LFPreview({p})")
                                }

                                /* ------------------------------------- */
                            }
                        }
                    }

                    impl std::str::FromStr for $enum {
                        type Err = String;

                        fn from_str(s: &str) -> Result<Self, Self::Err> {
                            let (name, data) = if let Some(pos) = s.find('(') {
                                if s.ends_with(')') {
                                    (&s[..pos], Some(&s[pos + 1..s.len() - 1]))
                                } else {
                                    (s, None)
                                }
                            } else {
                                (s, None)
                            };

                            match name {
                                $( stringify!($unit) => {
                                    if data.is_some() {
                                        Err(format!("Unexpected data for {}", name))
                                    } else {
                                        Ok(Self::$unit)
                                    }
                                }, )*

                                $( stringify!($tuple) => {
                                    let val = data
                                    .ok_or_else(|| format!("Missing data for {}", name))?
                                    .parse()
                                    .map_err(|_| format!("Invalid data for {}", name))?;
                                    Ok(Self::$tuple(val))
                                }, )*

                                $( stringify!($lossy) => {
                                    let d = match data {
                                        Some(val) => val.parse()
                                        .map_err(|_| format!("Invalid data for {}", stringify!($lossy)))?,
                                        None => Default::default(),
                                    };
                                    Ok(Self::$lossy(d))
                                }, )*

                                $( stringify!($default) => {
                                    let d = match data {
                                        Some(val) => val.parse()
                                        .map_err(|_| format!("Invalid data for {}", stringify!($default)))?,
                                        None => $default_value,
                                    };
                                    Ok(Self::$default(d))
                                }, )*

                                $( stringify!($optional) => {
                                    let d = match data {
                                        Some(val) if !val.is_empty() => {
                                            Some(val.parse().map_err(|_| format!("Invalid data for {}", stringify!($optional)))?)
                                        }
                                        _ => None,
                                    };
                                    Ok(Self::$optional(d))
                                }, )*

                                /* ---------- Manually parsed ---------- */
                                "Jump" => {
                                    let Some(values) = data else {
                                        return Ok(Self::Jump(vec![]))
                                    };
                                    let paths = cba::bring::split::split_on_unescaped_delimiter(values, ",").iter().map(PathBuf::from).collect();
                                    Ok(Self::Jump(paths))
                                }
                                "Lessfilter" => {
                                    let preset_str = data.ok_or_else(|| "Missing preset for Lessfilter")?;
                                    let preset = preset_str.to_lowercase().parse().map_err(|_| format!("Invalid preset for Lessfilter: {preset_str}"))?;
                                    Ok(FsAction::new_lessfilter(preset, false))
                                }
                                "LFPaged" => {
                                    let preset_str = data.ok_or_else(|| "Missing preset for LFPaged")?;
                                    let preset = preset_str.to_lowercase().parse().map_err(|_| format!("Invalid preset for LFPaged: {preset_str}"))?;
                                    Ok(FsAction::new_lessfilter(preset, true))
                                }
                                "LFPreview" => {
                                    let preset_str = data.ok_or_else(|| "Missing preset for LFPreview")?;
                                    let preset = preset_str.to_lowercase().parse().map_err(|_| format!("Invalid preset for LFPreview: {preset_str}"))?;
                                    let header = When::default();
                                    Ok(Self::LessfilterPreview ( preset, header ))
                                }
                                "Help" if data.is_none() => {
                                    Ok(FsAction::help())
                                }
                                "Execute" | "Exec" => {
                                    let cmd = data.ok_or_else(|| "Missing command for Execute")?;
                                    Ok(Self::Execute(cmd.into()))
                                }

                                /* ------------------------------------- */

                                _ => Err("".to_string()),
                            }
                        }
                    }
                };
            }
use enum_from_str_display;
