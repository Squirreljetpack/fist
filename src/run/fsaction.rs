//! note: Action handler.
//! State is managed externally: see [`super::global`] and [`super::thread_local`]

use std::{path::PathBuf, sync::atomic::Ordering};

use cli_boilerplate_automation::{bait::OptionExt, bath::PathExt, else_default};
use matchmaker::{
    acs,
    action::{Action, ActionExt, Actions, Count, Exit},
    efx,
    nucleo::{Color, Indexed, Modifier, Span, Style},
    render::{Effect, Effects},
};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::{home_dir, pager_path},
    clipboard::{copy_files, copy_paths_as_text},
    filters::SortOrder,
    lessfilter::Preset,
    run::{
        fspane::FsPane,
        item::short_display,
        stash::{STASH, StackItem},
        state::{APP, ENTERED_PROMPT, FILTERS, GLOBAL, PRINT_HANDLE, STACK, TEMP, TOAST},
    },
    spawn::open_wrapped,
    utils::text::{ToastStyle, format_cwd_prompt},
};

#[derive(Debug, Clone, strum_macros::Display, PartialEq)]
#[non_exhaustive]
pub enum FsAction {
    // Nav
    // ----------------------------------
    /// Enter a file or directory.
    /// (Default bind: Right).
    Advance,
    /// Enter the parent of the current directory.
    /// In the NAV pane, the current directory the one displayed when in the prompt.
    /// In other panes, this enters the parent of the current item.
    /// (Default bind: Left).
    Parent,
    /// Search in the current directory.
    /// (Default bind: ctrl-f).
    Find,
    /// Full text search in the current directory.
    /// (Default bind: ctrl-r).
    Rg,
    /// Search your visited directories/files.
    /// (Default bind: ctrl-g).
    History,
    /// Jump to a directory.
    /// # Note
    /// By default, '~' and '/' bind to Jump($HOME)
    Jump(String, char),

    /// Go back
    /// (Default bind: ctrl-z)
    Undo,
    /// Go forward
    /// (Default bind: alt-z)
    Forward,
    // nonbindable
    EnterPrompt(bool),
    // nonbindable
    SaveInput,

    // Display
    // ----------------------------------
    /// Display current filters.
    Filters,
    /// Display the current stack.
    Stack,
    /// Clear the stack.
    ClearStack,

    /// Show all* available actions on the current item(s).
    /// (E to interact).
    /// (not fully implemented)
    Menu,
    /// Toggle only showing directories
    /// In [`FsPane::Files`], [`FsPane::Folders`], [`FsPane::Launch`], this toggles their sort order
    ToggleDirs,
    ToggleHidden,

    // file actions
    // ----------------------------------
    /// Cut file (to stack).
    ///
    /// Also copies the file to the system clipboard.
    Cut,
    /// Copy file (to stack).
    ///
    /// Also copies the file to the system clipboard.
    Copy,
    /// Copy full path
    CopyPath,
    /// Create a new file. (todo)
    New,
    /// Stash file (to stack) in Symlink mode.
    Symlink,
    /// Save the file to the backup directory. (todo)
    Backup,
    /// Delete the file using system trash.
    Trash,
    /// Permanently delete the file. (todo: confirmation).
    Delete,
    /// Paste all stack items into the current or specified directory
    Paste(PathBuf), // dump Stack
    /// Execute according to [`crate::lessfilter::RulesConfig`]
    Handler(Preset, bool),
}
// print, accept

impl ActionExt for FsAction {}

// ------------- HANDLING ------------------

fn enter_dir_pane(path: AbsPath) {
    TOAST::clear_msgs();
    // record
    GLOBAL::db().bump(true, path.clone());
    // this happens after the reload, so that the config dependent prompt marker gets applied
    GLOBAL::send_efx(efx![Effect::RestoreInputPromptMarker]);
    // todo: somehow change the render inputui config
    // always clear

    // pane
    let pane = FsPane::new_nav(path, FILTERS::visibility(), FILTERS::sort());
    STACK::push(pane);
}

// note: since this happens before the batch process of actions, we do not support chaining custom actions
// i.e. "current" saved inputs in chained actions, or consecutive nav actions
pub fn fsaction_aliaser(
    a: Action<FsAction>,
    state: &MMState<'_>,
) -> Actions<FsAction> {
    #[allow(non_snake_case)]
    let RELOAD: Actions<FsAction> = acs![Action::ClearAll, Action::Reload("".to_string())];
    let raw_input = state.picker_ui.results.cursor_disabled || state.overlay_index.is_some();

    match a {
        Action::Custom(fa) => match fa {
            FsAction::Find => {
                // save input
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                STACK::save_input(content, index);

                // pane
                let pane = FsPane::new_fd(
                    STACK::cwd().unwrap_or_default(),
                    FILTERS::sort(),
                    FILTERS::visibility(),
                );
                STACK::push(pane);

                // we enter prompt because editing is likely
                acs![
                    Action::ClearAll,
                    Action::Reload("".to_string()),
                    Action::Custom(FsAction::EnterPrompt(true))
                ]
            }
            FsAction::History => {
                // save input
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                STACK::save_input(content, index);

                STACK::swap_history();

                acs![
                    Action::ClearAll,
                    Action::Reload("".to_string()),
                    Action::Custom(FsAction::EnterPrompt(true))
                ]
            }
            FsAction::Jump(d, c) => {
                if raw_input {
                    return acs![Action::Input(c)];
                }

                let path = d.abs(home_dir());
                let path = AbsPath::new_unchecked(&path);

                if Some(&path) == STACK::cwd().as_ref() {
                    return acs![];
                }

                if !path.is_dir() {
                    TOAST::push_msg(
                        vec![
                            Span::styled(d, Color::Red),
                            Span::raw(" is not a directory!"),
                        ],
                        false,
                    );
                    return acs![];
                }

                // save input
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                STACK::save_input(content, index);

                // pane
                enter_dir_pane(path);

                RELOAD
            }
            FsAction::Parent => {
                if raw_input {
                    return acs![Action::BackwardChar];
                }
                if APP::in_app_pane() {
                    // todo!()
                    return acs![];
                }

                // get parent path
                let cwd = STACK::cwd();

                // If Nav, go to the parent of the cwd, otherwise go to the parent of the current item,
                let path = if STACK::with_current(|x| matches!(x, FsPane::Nav { .. })) {
                    else_default!(
                        cwd.as_ref()
                            .and_then(|x| x.parent().map(AbsPath::new_unchecked))
                    )
                } else {
                    else_default!(
                        state
                            .current
                            .as_ref()
                            .and_then(|(_, x)| x.path.parent().map(AbsPath::new_unchecked))
                    )
                };

                // save current for lookup
                TEMP::set_prev_dir(cwd);
                // save input
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                STACK::save_input(content, index);

                // pane
                enter_dir_pane(path);

                RELOAD
            }
            FsAction::Advance => {
                if raw_input {
                    return acs![Action::ForwardChar];
                }
                let Some(Indexed { inner: item, .. }) = state.current_raw() else {
                    return acs![];
                };
                if APP::in_app_pane() {
                    // todo!()
                    return acs![];
                }

                if item.path.is_dir() {
                    // save input
                    let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                    STACK::save_input(content, index);

                    // pane
                    enter_dir_pane(item.path.clone());

                    RELOAD
                } else if item.path.exists() {
                    // record
                    if item.path.is_file() {
                        GLOBAL::db().bump(false, item.path.clone());
                    }

                    acs![Action::Execute(GLOBAL::with_cfg(|c| c
                        .interface
                        .advance_cmd
                        .clone())),]
                } else {
                    acs![]
                }
            }
            FsAction::Undo => {
                // save input
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                STACK::save_input(content, index);

                // adjust stack
                if STACK::stack_prev() { RELOAD } else { acs![] }
            }
            FsAction::Forward => {
                // save input
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                STACK::save_input(content, index);

                // adjust stack
                if STACK::stack_next() {
                    // restore input
                    RELOAD
                } else {
                    acs![]
                }
            }
            FsAction::SaveInput => {
                let (content, index) = (state.input.clone(), state.picker_ui.results.index());
                log::debug!("Saved: {content}, {index}");
                STACK::save_input(content, index);

                acs![]
            }
            //  ------------- Overlay aliases --------------
            FsAction::Stack => {
                acs![Action::Overlay(0)]
            }
            FsAction::Filters => {
                acs![Action::Overlay(1)]
            }
            FsAction::Menu => {
                if let Some((_, ref p)) = state.current
                    && !raw_input
                {
                    TEMP::set_prompt(None, p.clone());
                    acs![Action::Overlay(2)]
                } else {
                    acs![]
                }
            }
            // FsAction::Category => {
            //     acs![Action::Overlay(3)]
            // }
            FsAction::Handler(p, page) => {
                let mut cmd = p.to_command_string();
                if page {
                    let pp = else_default!(
                        pager_path()
                            .to_str()
                            .elog("Invalid encoding for pager path")
                            .ok()
                    );
                    cmd.push_str(" | ");
                    cmd.push_str(pp);
                }
                acs![Action::Execute(cmd)]
            }
            _ => acs![Action::Custom(fa)],
        },
        _ => match a {
            Action::Up(Count(i)) => {
                TOAST::clear();

                if state.overlay_index.is_some() {
                    acs![a]
                } else if state.picker_ui.results.cursor_disabled {
                    if ENTERED_PROMPT
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::SeqCst)
                        .is_ok()
                    {
                        acs![
                            Action::Custom(FsAction::EnterPrompt(false)),
                            Action::Up(Count(i))
                        ]
                    } else {
                        acs![a]
                    }
                } else if i as u32 <= state.picker_ui.results.index() {
                    acs![a]
                } else {
                    // entering the prompt
                    if ENTERED_PROMPT
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::SeqCst)
                        .is_ok()
                    {
                        acs![Action::Custom(FsAction::EnterPrompt(true))]
                    } else {
                        acs![]
                    }
                }
            }
            Action::Down(Count(i)) => {
                TOAST::clear();

                if state.overlay_index.is_none()
                    && state.picker_ui.results.cursor_disabled
                    && ENTERED_PROMPT
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::SeqCst)
                        .is_ok()
                {
                    acs![Action::Custom(FsAction::EnterPrompt(false))]
                } else {
                    acs![a]
                }
            }
            // We treat Print("") special, and comparably to Accept
            // It prints elements on seperate lines
            // then exits afterwards
            // The intention is to feed into a shell function
            // Might make more sense as a custom action
            Action::Print(ref s) if s.is_empty() => {
                if state.picker_ui.results.cursor_disabled
                    && let Some(p) = STACK::cwd()
                {
                    let s = p.to_string_lossy().to_string();
                    GLOBAL::db().bump(true, p);
                    PRINT_HANDLE.with(|x| x.push(s));
                } else {
                    state.map_selected_to_vec(|item| {
                        let s = item.display().to_string();
                        GLOBAL::db().bump(item.path.is_dir(), item.path.clone());
                        PRINT_HANDLE.with(|x| x.push(s));
                    });
                }
                acs![Action::Quit(Exit(0))]
            }
            // accepting on prompt opens the displayed directory
            Action::Accept
                if state.picker_ui.results.cursor_disabled && state.overlay_index.is_none() =>
            {
                if let FsPane::Nav { cwd, .. } = STACK::current() {
                    let path = cwd.inner().into();
                    let pool = GLOBAL::db();

                    tokio::spawn(async move {
                        let conn = pool.get_conn(crate::db::DbTable::dirs).await?;
                        open_wrapped(conn, None, &[path]).await?;
                        anyhow::Ok(())
                    });
                    acs![a]
                } else {
                    let cwd = STACK::cwd(); // strange that the lock stays alive through the owned value lifetime
                    if let Some(cwd) = cwd {
                        // save input
                        let (content, index) =
                            (state.input.clone(), state.picker_ui.results.index());
                        STACK::save_input(content, index);

                        enter_dir_pane(cwd);

                        acs![Action::Reload("".into()),]
                    } else {
                        acs![]
                    }
                }
            }
            _ => acs![a],
        },
    }
}

pub fn fsaction_handler(
    a: FsAction,
    state: &MMState<'_>,
) -> Effects {
    match a {
        // nonbindable
        FsAction::EnterPrompt(enter) => {
            // finish debounce
            ENTERED_PROMPT.store(false, Ordering::SeqCst);

            // set prompt
            if enter {
                if let Some(cwd) = STACK::cwd() {
                    let content = format_cwd_prompt(
                        &GLOBAL::with_cfg(|c| c.interface.cwd_prompt.clone()),
                        &cwd,
                    );
                    let prompt = Span::styled(
                        content,
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::ITALIC),
                    );
                    efx![Effect::DisableCursor(enter), Effect::Prompt(prompt)]
                } else {
                    efx![]
                }
            } else {
                efx![
                    Effect::DisableCursor(enter),
                    Effect::RestoreInputPromptMarker
                ]
            }
        }

        // File actions
        // --------------------------------
        // todo: if cursor_disabled, used STACK::cwd
        FsAction::Cut => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            STASH::insert(state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                cb_vec.push(s.path.inner());
                StackItem::mv(s.path.clone())
            }));
            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Cut: ", toast_vec);
                copy_files(cb_vec, false);
            };
            efx![]
            // if let Some(c) = state.current_raw() {
            //     scratch_toggle(StackItem::mv(c.inner.path.clone()) );
            // }
        }
        FsAction::Copy => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            STASH::insert(state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                cb_vec.push(s.path.inner());
                StackItem::cp(s.path.clone())
            }));
            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Copied: ", toast_vec);
                copy_files(cb_vec, false);
            };
            efx![]
        }
        FsAction::Symlink => {
            let mut toast_vec = vec![];
            STASH::insert(state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                StackItem::cp(s.path.clone())
            }));
            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Stashed: ", toast_vec);
            };
            efx![]
        }
        FsAction::Backup => {
            todo!();
        }
        FsAction::Trash => {
            let mut items = vec![];
            state.map_selected_to_vec(|s| {
                items.push(s.path.inner());
            });
            tokio::spawn(async {
                for i in items {
                    match trash::delete(&i) {
                        Ok(()) => {
                            TOAST::push(ToastStyle::Success, "Trashed: ", [short_display(&i)]);
                        }
                        Err(e) => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to trash: ",
                                [short_display(&i)],
                            );
                        }
                    }
                }
            });
            efx![]
        }
        FsAction::Delete => {
            let mut items = vec![];
            state.map_selected_to_vec(|s| {
                items.push(s.path.inner());
            });

            tokio::spawn(async move {
                for i in items {
                    let result = if i.is_dir() {
                        std::fs::remove_dir_all(&i)
                    } else {
                        std::fs::remove_file(&i)
                    };

                    match result {
                        Ok(()) => {
                            TOAST::push(ToastStyle::Success, "Deleted: ", [short_display(&i)]);
                        }
                        Err(e) => {
                            TOAST::push(
                                ToastStyle::Error,
                                "Failed to delete: ",
                                [short_display(&i)],
                            );
                        }
                    }
                }
            });

            efx![]
        }
        FsAction::CopyPath => {
            let paths = if !state.picker_ui.results.cursor_disabled {
                state.map_selected_to_vec(|s| s.path.inner())
            } else {
                STACK::cwd().map(PathBuf::from).into_iter().collect()
            };

            copy_paths_as_text(paths, true);
            efx![]
        }
        FsAction::Paste(dest_base) => {
            let base = if dest_base.is_empty() {
                if let Some(c) = STACK::nav_cwd() {
                    c
                } else {
                    TOAST::push_notice(ToastStyle::Normal, "No current directory.");
                    return efx![];
                }
            } else {
                if !dest_base.is_absolute() {
                    TOAST::push_notice(
                        ToastStyle::Error,
                        format!("{} is not absolute.", dest_base.to_string_lossy()),
                    );
                    return efx![];
                }
                AbsPath::new_unchecked(dest_base)
            };
            STASH::transfer_all(&base, true);
            efx![]
        }
        FsAction::ClearStack => {
            STASH::clear_invalid_and_completed();
            TOAST::push_notice(ToastStyle::Normal, "Stack cleared");
            efx![]
        }

        // filters
        FsAction::ToggleDirs => {
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
                FILTERS::refilter();

                if !p_str.is_empty() {
                    let prompt = Span::styled(
                        p_str,
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::ITALIC),
                    );
                    efx![Effect::Prompt(prompt)]
                } else {
                    efx![Effect::RestoreInputPromptMarker]
                }
            } else if let Some(e) = FILTERS::with_mut(|_sort, vis| {
                (vis.dirs, vis.files) = match (vis.dirs, vis.files) {
                    (false, false) => (true, false),
                    (true, false) => (false, true),
                    (false, true) => (false, false),
                    (true, true) => {
                        log::error!("Unexpected toggle dirs state");
                        (false, false)
                    }
                };
                if !state.picker_ui.results.cursor_disabled {
                    if vis.dirs {
                        Some(Effect::Prompt(Span::styled(
                            "d: ",
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::ITALIC),
                        )))
                    } else if vis.files {
                        Some(Effect::Prompt(Span::styled(
                            "f: ",
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::ITALIC),
                        )))
                    } else {
                        Some(Effect::RestoreInputPromptMarker)
                    }
                } else {
                    None
                }
            }) {
                FILTERS::refilter();
                efx![e]
            } else {
                FILTERS::refilter();
                efx![]
            }
        }
        FsAction::ToggleHidden => {
            FILTERS::with_mut(|_sort, vis| {
                let style = Style::new().add_modifier(Modifier::DIM).italic();
                if vis.hidden || vis.all() {
                    vis.set_default();
                    TOAST::push_msg(Span::styled("Default filters", style), true);
                } else {
                    vis.hidden = true;
                    TOAST::push_msg(Span::styled("Showing hidden", style), true);
                }
            });
            FILTERS::refilter();
            efx![]
        }

        _ => unreachable!(),
    }
}

// ----------------------------
pub fn paste_handler(
    content: String,
    state: &MMState<'_>,
) -> String {
    if GLOBAL::with_cfg(|c| c.interface.always_paste) || state.picker_ui.results.cursor_disabled {
        content
    } else {
        // paste action
        let base = if let Some(c) = STACK::nav_cwd() {
            c
        } else {
            TOAST::push_notice(ToastStyle::Normal, "No current directory.");
            return String::new();
        };
        STASH::transfer_all(&base, true);
        String::new()
    }
}

// ------------- BOILERPLATE ---------------
impl std::str::FromStr for FsAction {
    type Err = ();

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        Err(())
    }
}
