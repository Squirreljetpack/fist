//! note: Action handler.
//! State is managed externally: see [`super::global`] and [`super::thread_local`]

use std::path::PathBuf;

use cba::{bait::ResultExt, bath::PathExt, bring::split::join_with_single_quotes, unwrap, wbog};
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
        ahandlers::{enter_dir_pane, enter_prompt, fs_reload},
        item::short_display,
        pane::FsPane,
        stash::STASH,
        state::{
            ExecuteHandlerShouldProcessParent, FILTERS, GLOBAL, ShouldNotAbortOnEmpty, STACK,
            TASKS, TOAST, TlsStore, context::ActionContext,
        },
    },
    spawn::open_wrapped,
    ui::{
        confirm_overlay::ConfirmPrompt,
        menu_overlay::{MenuTarget, PromptKind},
    },
    utils::text::ToastStyle,
};
use fist_types::When;
use fist_types::filters::SortOrder;

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
    App, // if u want to push selections, u can compose clearstack/pushstack before/after

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
    ClearStash(Option<String>), // if empty, clear shared. if name, clear specific kind.

    CycleStash(bool),
    SwitchStash(String),
    Stash(String),
    ShowExclusiveStash,

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
    /// Create a new file.
    New,
    /// Create a new directory. (todo)
    NewDir,
    /// Rename a file or directory.
    Rename,

    /// Save the file to the backup directory. (todo)
    Backup,
    /// Delete the file using system trash.
    Trash,
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
    // Execute
    Execute(String, usize),
    // Nonbindable
    // ----------------------------------
    EnterPrompt(bool),
    SaveInput,
    SetHeader(Option<Text<'static>>),
    SetFooter(Option<Text<'static>>),
    Reload,
    AcceptPrompt,
    AcceptPrint,
    Filtering(Option<bool>),
    SetStatus(Option<Line<'static>>),

    // Other
    // ----------------------------------
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
    let in_prompt = state.picker_ui.results.cursor_disabled;
    let raw_input = in_prompt || state.overlay_index().is_some();

    match a {
        Action::Custom(fa) => match fa {
            // handle nonbindable events here so that overlays don't intercept them.
            // -------------------------------------------------
            FsAction::Reload => {
                fs_reload(state, false);
                acs![]
            }
            FsAction::SaveInput => {
                let (content, index) = (
                    state.picker_ui.query.input.clone(),
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
                    state.filtering = s
                } else {
                    state.filtering = !state.filtering
                };
                acs![]
            }
            FsAction::SetStatus(s) => {
                state.picker_ui.results.set_status_line(s);
                acs![]
            }
            FsAction::EnterPrompt(enter) => {
                enter_prompt(state, enter);
                acs![]
            }

            // Actions which only trigger when not in the prompt:
            // -------------------------------------------------
            FsAction::Parent => {
                if raw_input {
                    acs![Action::BackwardChar]
                } else if STACK::in_app() {
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
                // probably not a good idea to put a delete action on the same key
                // if raw_input {
                //     acs![Action::DeleteWord]
                // } else
                if STACK::in_app() {
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }
            FsAction::Trash => {
                // if raw_input {
                //     acs![Action::DeleteWord]
                // } else
                if STACK::in_app() {
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }

            //  ------------- Overlay aliases --------------
            FsAction::ShowStash
            | FsAction::ShowExclusiveStash
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
            FsAction::ShowExclusiveStash => {
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
                if let Some(p) = state.current_item() {
                    TlsStore::set_input_bar(None, MenuTarget::Item(p.clone()));
                    acs![Action::Overlay(4)]
                } else if let Some(cwd) = STACK::cwd() {
                    TlsStore::set_input_bar(None, MenuTarget::Cwd(cwd));
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
                if let Some(p) = state.current_raw() {
                    let p = p.path._parent();
                    TlsStore::set_input_bar(Some(PromptKind::NewDir), MenuTarget::Cwd(p));
                    acs![Action::Overlay(3)]
                } else if let Some(cwd) = STACK::nav_cwd() {
                    TlsStore::set_input_bar(Some(PromptKind::NewDir), MenuTarget::Cwd(cwd));
                    acs![Action::Overlay(3)]
                } else {
                    acs![]
                }
            }
            // FsAction::NewDir => {
            //     // undecided
            // }

            // FsAction::Category => {
            //     acs![Action::Overlay(3)]
            // }
            FsAction::AutoJump(digit) => {
                if state.overlay_index().is_some()
                // in overlay
                {
                    acs![Action::Pos(digit.saturating_sub(1) as i32)]
                } else if in_prompt
                // in prompt
                {
                    // jump out
                    if digit > 0 {
                        enter_prompt(state, false);
                        acs![Action::Pos(digit as i32 - 1)]
                    } else {
                        acs![FsAction::AcceptPrompt]
                    }
                } else if digit == 0
                // 0 when not in prompt -> enter prompt
                {
                    enter_prompt(state, true);
                    acs![]
                } else if (digit - 1) as u32 == state.picker_ui.results.index()
                // not in prompt => accept
                {
                    acs![
                        Action::Pos((digit - 1) as i32),
                        if GLOBAL::with_cfg(|c| c.interface.autojump_advance) {
                            FsAction::Advance.into()
                        } else if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                            FsAction::AcceptPrint.into()
                        } else {
                            Action::Accept
                        }
                    ]
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
                } else if in_prompt {
                    enter_prompt(state, false);
                    if !state.picker_ui.reverse() {
                        acs![a]
                    } else {
                        acs![Action::Up(i.saturating_sub(1))]
                    }
                } else if i as u32 > state.picker_ui.results.index() && !state.picker_ui.reverse() {
                    // entering the prompt
                    enter_prompt(state, true);
                    acs![]
                } else {
                    acs![a]
                }
            }
            Action::Down(i) => {
                TOAST::clear();

                if state.overlay_index().is_some() {
                    acs![a]
                } else if in_prompt {
                    enter_prompt(state, false);
                    if state.picker_ui.reverse() {
                        acs![a]
                    } else {
                        acs![Action::Down(i.saturating_sub(1))]
                    }
                } else if i as u32 > state.picker_ui.results.index() && state.picker_ui.reverse() {
                    // entering the prompt
                    enter_prompt(state, true);
                    acs![]
                } else {
                    acs![a]
                }
            }
            Action::Pos(_) if state.overlay_index().is_none() && in_prompt => {
                enter_prompt(state, false);
                acs![a]
            }

            // there's a bit of an edge case where this doesn't detect whether to be in prompt correctly for consecutive actions but for expediency we'll leave as is
            Action::Accept => {
                if state.overlay_index().is_some() {
                    acs![a]
                } else if in_prompt {
                    acs![FsAction::AcceptPrompt]
                } else if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                    acs![FsAction::AcceptPrint]
                } else {
                    acs![Action::Accept]
                }
            }

            Action::Print(ref s) if s.is_empty() => {
                if state.overlay_index().is_some() {
                    acs![a]
                } else if in_prompt {
                    acs![FsAction::AcceptPrompt]
                } else if !GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                    acs![FsAction::AcceptPrint]
                } else {
                    acs![Action::Accept]
                }
            }

            Action::Reload(s)
                if s.is_empty() && STACK::with_current(|c| matches!(c, FsPane::Stream { .. })) =>
            {
                TOAST::msg("Cannot reload streams", false);
                acs![]
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
    let in_prompt = state.picker_ui.results.cursor_disabled;

    match a {
        FsAction::Find => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // pane
            let pane = FsPane::new_fd(
                STACK::cwd().unwrap_or_default(),
                FILTERS::sort(),
                FILTERS::visibility(),
            );

            TlsStore::set(ShouldNotAbortOnEmpty {});

            // don't push if same pane: changes in filter/vis already should be the ones to responsible for that (todo?)
            // todo: there is a problem
            if STACK::with_current(|p| *p == pane) {
                fs_reload(state, false);
            } else {
                STACK::push(pane);
                fs_reload(state, true);
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

            fs_reload(state, true);
        }

        FsAction::Search => {
            if STACK::with_current_mut(|x| match x {
                FsPane::Search {
                    input,
                    filtering,
                    patterns,
                    ..
                } => {
                    if patterns.is_empty() {
                        patterns.push(String::new());
                    }

                    *filtering = !*filtering;

                    let new_input = if *filtering {
                        // entering filter:
                        // restore from input
                        &input.0
                    } else {
                        // entering rg:

                        // save input
                        *input = state.get_content_and_index();
                        // set picker.input to previous
                        &join_with_single_quotes(patterns)
                    };

                    state.picker_ui.query.set(new_input.clone(), u16::MAX);

                    true
                }
                _ => false,
            }) {
                fs_reload(state, false);
            } else {
                // save input
                let (content, index) = state.get_content_and_index();
                STACK::save_input(content, index);

                let [one_line, fixed_strings] =
                    GLOBAL::with_cfg(|c| [c.panes.search.one_line, c.panes.search.fixed_strings]);

                let cwd = STACK::cwd().unwrap_or_default();

                //
                let paths = vec![cwd.inner()];
                let query = String::new();
                let filtering = false;
                let patterns = if GLOBAL::with_cfg(|c| c.panes.search.search_empty_query) {
                    vec!["".into()]
                } else {
                    vec![]
                };

                // let filtering = !(patterns.is_empty() || patterns[0].is_empty());
                let context = Default::default();
                let case = Default::default();

                let pane = FsPane::new_rg_full(
                    cwd,
                    FILTERS::sort(),
                    FILTERS::visibility(),
                    //
                    paths,
                    String::new(),
                    patterns,
                    filtering,
                    //
                    context,
                    case,
                    one_line,
                    fixed_strings,
                    vec![],
                );
                STACK::push(pane);
                fs_reload(state, true);
            }
        }

        FsAction::App => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            TlsStore::set(STASH::current_exclusive());

            let pane = FsPane::new_launch();
            if STACK::set_or_push(pane) {
                fs_reload(state, true);
            } else {
                fs_reload(state, false);
            }
        }

        FsAction::Undo => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // adjust stack
            if STACK::stack_prev() {
                fs_reload(state, true);
            };
        }
        FsAction::Redo => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // adjust stack
            if STACK::stack_next() {
                fs_reload(state, true);
            };
        }

        FsAction::Jump(paths) => {
            let cwd = STACK::cwd().and_then(|p| p.canonicalize().ok());

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

            let target_path = if paths.is_empty() {
                __home().into()
            } else {
                paths[idx].abs(__home())
            };

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
            // get parent path
            let cwd = STACK::cwd();

            // If Nav, go to the parent of the cwd, otherwise go to the parent of the current item,
            let path = if STACK::with_current(|x| matches!(x, FsPane::Nav { .. })) {
                unwrap!(
                    cwd.as_ref()
                        .and_then(|x| x.parent().map(AbsPath::new_unchecked))
                )
            } else {
                unwrap!(
                    state
                        .current_raw()
                        .and_then(|x| x.path.parent().map(AbsPath::new_unchecked))
                )
            };

            // save current for lookup
            TlsStore::maybe_set(cwd);
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
                    GLOBAL::db().bump(false, item.path.clone());
                }

                // todo: specialized
                let template = GLOBAL::with_cfg(|c| c.interface.advance_command.clone());
                state.set_interrupt(Interrupt::Execute, template);
            }
        }

        // File actions
        // --------------------------------
        // todo: if cursor_disabled, used STACK::cwd
        FsAction::Cut => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            let items = state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                cb_vec.push(s.path.inner());
                s.path.clone()
            });
            if !items.is_empty() {
                STASH::extend("cut", items);
                TOAST::push(ToastStyle::Normal, "Cut: ", toast_vec);
                copy_files(cb_vec, false);
            };
        }
        FsAction::Copy => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            let items = state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                cb_vec.push(s.path.inner());
                s.path.clone()
            });
            if !items.is_empty() {
                STASH::extend("copy", items);
                TOAST::push(ToastStyle::Normal, "Copied: ", toast_vec);
                copy_files(cb_vec, false);
            };
        }

        // Note: This is the only stash action which also pushes the cwd
        FsAction::Push => {
            let mut toast_vec = vec![];

            if !in_prompt {
                let items = state.map_selected_to_vec(|s| {
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

        FsAction::Stash(mode) => {
            let mut toast_vec = vec![];
            let mode = if mode.is_empty() {
                STASH::current_exclusive()
            } else {
                mode
            };
            if !in_prompt {
                let items = state.map_selected_to_vec(|s| {
                    toast_vec.push(short_display(&s.path));
                    s.path.clone()
                });
                if !items.is_empty() {
                    STASH::extend(&mode, items);
                }
            } else if let Some(p) = STACK::cwd() {
                toast_vec.push(short_display(&p));
                STASH::stash(&mode, p);
            };
            if !toast_vec.is_empty() {
                let mut line = Line::from(vec![Span::styled(
                    format!("Stashed ({}): ", mode),
                    ToastStyle::Normal,
                )]);
                line.spans.extend(toast_vec);
                TOAST::msg(line, false);
            };
        }

        FsAction::CycleStash(forwards) => {
            STASH::cycle_exclusive(forwards);
            TOAST::notice(
                ToastStyle::Normal,
                format!("EStash: {}", STASH::current_exclusive()),
            );
        }
        FsAction::SwitchStash(s) => {
            // todo!();
            // TOAST::push_notice(
            //     ToastStyle::Normal,
            //     format!("EStash: {}", STASH::current_exclusive()),
            // );
        }

        FsAction::Backup => {
            // todo: impl using custom stash + some kind of db-based kv store
        }

        FsAction::Trash => {
            let mut items = vec![];
            state.map_selected_to_vec(|s| {
                items.push(s.path.inner());
            });
            // not heavy computationally, but still blocking...
            TASKS::spawn_blocking(|| {
                for path in items {
                    match trash::delete(&path) {
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
            let mut items = vec![];
            state.map_selected_to_vec(|s| {
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

                TlsStore::set(ConfirmPrompt {
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
            let paths = if !in_prompt {
                state.map_selected_to_vec(|s| s.path.inner())
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
            STASH::transfer_all(base, false, None);
        }
        FsAction::ClearStash(x) => {
            STASH::clear(x.as_deref());

            TOAST::notice(ToastStyle::Normal, "Stack cleared");
        }
        // filters
        FsAction::FsToggle => {
            if STACK::with_current(|p| matches!(p, FsPane::Files { .. } | FsPane::Folders { .. })) {
                let p_str = FILTERS::with_mut(|sort, _vis| {
                    sort.cycle();
                    match sort {
                        SortOrder::mtime => "atime: ",
                        SortOrder::name => "name: ",
                        SortOrder::none => "score: ",
                        _ => "score: ",
                    }
                });

                if !p_str.is_empty() {
                    let prompt = Line::styled(
                        p_str,
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::ITALIC),
                    );
                    state.picker_ui.query.set_prompt_line(prompt);
                } else {
                    state.picker_ui.query.set_prompt(None);
                }
            } else {
                FILTERS::with_mut(|_sort, vis| {
                    (vis.dirs, vis.files) = match (vis.dirs, vis.files) {
                        (false, false) => (false, true),
                        (false, true) => (true, false),
                        (true, false) => (false, false),
                        (true, true) => {
                            log::error!("Unexpected toggle dirs state");
                            (false, false)
                        }
                    };
                    if !in_prompt {
                        if vis.dirs {
                            state
                                .picker_ui
                                .query
                                .set_prompt_line(Line::styled("d: ", prompt_main_style()));
                        } else if vis.files {
                            state
                                .picker_ui
                                .query
                                .set_prompt_line(Line::styled("f: ", prompt_main_style()));
                        } else {
                            state.picker_ui.query.set_prompt(None);
                        }
                    }
                });
            }
            FILTERS::refilter();
        }
        FsAction::ToggleHidden => {
            FILTERS::with_mut(|_sort, vis| {
                let style = Style::new().add_modifier(Modifier::DIM).italic();
                if vis.hidden || vis.all() {
                    vis.set_default();
                    TOAST::msg(Span::styled("Default filters", style), true);
                } else {
                    vis.hidden = true;
                    TOAST::msg(Span::styled("Showing hidden", style), true);
                }
            });
            FILTERS::refilter();
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

            if state.current_raw().is_none() && !in_prompt {
                return;
            };

            // since in Nav pane, Advance is bound to edit cursor item, it's more useful to make the action always edit the menu item.
            if matches!(preset, Preset::Edit)
                && state.current_raw().is_some_and(|x| x.path.is_file())
            {
                TlsStore::set(ExecuteHandlerShouldProcessParent {});
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
                if let Some(pp) = text_renderer_path().shell_quote() {
                    #[cfg(windows)]
                    template.push_str(&format!(" | cmd /c \"set PG_LANG=ini && {pp}\" > CON"));
                    #[cfg(unix)]
                    template.push_str(&format!(" | PG_LANG=ini {pp} > /dev/tty"));
                } else {
                    wbog!(
                        "Pager path could not be decoded, please check your installation's cache directory."
                    )
                }
            }

            state.set_interrupt(Interrupt::Execute, template);
        } // todo: use a special sequence to communicate to handler whether to pipe/silent or detach

        // See [`crate::run::dhandlers::execute`]
        FsAction::Execute(mut template, v) => {
            let prefix = format!("\0\0\0{}", v);
            template.insert_str(0, &prefix);

            if v == 2 || v == 3 {
                state.set_interrupt(Interrupt::ExecuteSilent, template);
            } else {
                state.set_interrupt(Interrupt::Execute, template);
            }
        }

        FsAction::AcceptPrompt => {
            if let Some(p) = STACK::nav_cwd() {
                if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                    // same as below
                    let s = p.display().to_string();
                    print_handle.push(s);

                    GLOBAL::db().bump(true, p);

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

        FsAction::AcceptPrint => {
            let pool = GLOBAL::db();
            if in_prompt && let Some(p) = STACK::cwd() {
                // print cwd
                let s = p.to_string_lossy().to_string();
                print_handle.push(s);

                // bump
                GLOBAL::db().bump(true, p);
            } else {
                // if alt_accept, this was aliased from Accept, in which case we should respect no_multi_accept
                if GLOBAL::with_cfg(|c| c.interface.alt_accept && c.interface.no_multi_accept) {
                    if let Some(item) = state.current_raw() {
                        GLOBAL::db().bump(item.path.is_dir(), item.path.clone());

                        let s = item.display().to_string();
                        print_handle.push(s);
                    }
                } else {
                    // print selected
                    let v = state.map_selected_to_vec(|item| {
                        GLOBAL::db().bump(item.path.is_dir(), item.path.clone());

                        let s = item.display().to_string();
                        print_handle.push(s);
                    });
                }
            }
            state.picker_ui.selector.clear();
            state.should_quit = true;
        }

        _ => {
            log::error!("Encountered unreachable {a:?}");
            unreachable!()
        }
    }
}

// ------------- BOILERPLATE ---------------
enum_from_str_display! {
    FsAction;

    units:
    Advance, Parent, Find, Search, History, App,
    Undo, Redo, Push,
    ShowFilters, ShowStash, ShowExclusiveStash,
    ShowMenu, FsToggle, ToggleHidden,
    Cut, Copy, CopyPath, New, NewDir, Rename,
    Backup, Trash;

    tuples:
    AutoJump, SwitchStash;

    defaults:
    (Delete, false), (Stash, String::new()), (CycleStash, true)
    ;
    options:
    ClearStash
    ;

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
                                    write!(f, "Jump(⌂)")
                                } else {
                                    write!(f, "Jump({})", paths
                                    .iter()
                                    .map(|p| p.to_string_lossy())
                                    .collect::<Vec<_>>()
                                    .join("|||")
                                )
                            }
                        }
                        SaveInput | SetHeader(_) | SetFooter(_) | Reload | AcceptPrompt | AcceptPrint | Filtering(_) | SetStatus(_) | EnterPrompt(_) | Confirm => Ok(()), // internal
                        Lessfilter { preset, paging, header: _, .. } => {
                            let mut preset = preset.to_string();
                            if *paging {
                                preset.push('|')
                            };
                            write!(f, "Lessfilter({preset})")
                        },
                        Execute(s, u) => {
                            match u {
                                1 => write!(f, "ExecutePaged({})", s),
                                2 => write!(f, "ExecuteDetached({})", s),
                                3 => write!(f, "ExecuteSilent({})", s),
                                4 => write!(f, "ExecuteTTY({})", s),
                                _ => write!(f, "Execute({})", s),
                            }
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
                            let values = data.ok_or_else(|| "Missing path for Jump")?;
                            let paths = cba::bring::split::split_on_unescaped_delimiter(values, "|||").iter().map(PathBuf::from).collect();
                            Ok(Self::Jump(paths))
                        }
                        "Lessfilter" => {
                            let mut paging = false;
                            let mut preset_str = data.ok_or_else(|| "Missing preset for Lessfilter")?;

                            if let Some(stripped) = preset_str.strip_suffix('|') {
                                preset_str = stripped;
                                paging = true;
                            }

                            let preset = preset_str.to_lowercase().parse().map_err(|_| format!("Invalid preset for lessfilter: {preset_str}"))?;
                            let header = When::default();
                            Ok(Self::Lessfilter { preset, paging, header, special: Default::default() })
                        }
                        "Execute" => {
                            let cmd = data.ok_or_else(|| "Missing command for Execute")?;
                            Ok(Self::Execute(cmd.into(), 0))
                        }
                        "ExecutePaged" => {
                            let cmd = data.ok_or_else(|| "Missing command for ExecutePaged")?;
                            Ok(Self::Execute(cmd.into(), 1))
                        }
                        "ExecuteDetached" => {
                            let cmd = data.ok_or_else(|| "Missing command for ExecuteDetached")?;
                            Ok(Self::Execute(cmd.into(), 2))
                        }
                        "ExecuteSilent" => {
                            let cmd = data.ok_or_else(|| "Missing command for ExecuteSilent")?;
                            Ok(Self::Execute(cmd.into(), 3))
                        }
                        "ExecuteTTY" => {
                            let cmd = data.ok_or_else(|| "Missing command for ExecuteTTY")?;
                            Ok(Self::Execute(cmd.into(), 4))
                        }

                        /* ------------------------------------- */

                        _ => Err(format!("Unknown action {}", s)),
                    }
                }
            }
        };
    }
use enum_from_str_display;

pub fn prompt_main_style() -> Style {
    Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::ITALIC)
}
