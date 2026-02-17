//! note: Action handler.
//! State is managed externally: see [`super::global`] and [`super::thread_local`]

use std::path::PathBuf;

use cli_boilerplate_automation::{
    bait::ResultExt, bath::PathExt, bother::types::When, prints, unwrap, wbog,
};
use matchmaker::{
    acs,
    action::{Action, Actions},
    message::Interrupt,
    nucleo::{Color, Modifier, Span, Style},
};
use ratatui::text::Text;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::{__home, text_renderer_path},
    clipboard::{copy_files, copy_paths_as_text},
    filters::SortOrder,
    lessfilter::Preset,
    run::{
        ahandler::{enter_dir_pane, enter_prompt, fs_reload, prepare_prompt},
        item::short_display,
        pane::FsPane,
        stash::{STASH, StashItem},
        state::{APP, FILTERS, GLOBAL, STACK, TASKS, TEMP, TOAST},
    },
    spawn::open_wrapped,
    ui::menu_overlay::PromptKind,
    utils::text::ToastStyle,
};

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
    Rg,
    /// Search your visited directories/files.
    History,
    /// Jump to a directory.
    /// Relative paths are resolved relative to the home directory.
    ///
    /// # Note
    /// The char is emitted instead of jumping if the index is in the prompt.
    Jump(PathBuf, Option<char>),

    /// Go back
    Undo,
    /// Go forward
    Redo,

    // Display
    // ----------------------------------
    /// Display current filters.
    Filters,
    /// Display the current stack.
    Stash,
    /// Clear the stack.
    ClearStash,

    /// Show available actions on the current item(s).
    Menu,
    /// Toggle only showing directories.
    /// In [`FsPane::Files`], [`FsPane::Folders`], [`FsPane::Launch`], this toggles their sort order.
    ToggleDirs,
    /// Toggle showing hidden files.
    ToggleHidden,

    // file actions
    // ----------------------------------
    /// Cut file (to the [`STASH`] and the system clipboard).
    Cut,
    /// Copy file (to the [`STASH`] and the system clipboard).
    Copy,
    /// Copy full path.
    CopyPath,
    /// Create a new file.
    New,
    /// Create a new directory. (todo)
    NewDir,
    /// Save a file to the [`STASH`] under the [`Symlink`](crate::run::stash::StashAction::Symlink) action.
    Symlink,
    /// Save the file to the backup directory. (todo)
    Backup,
    /// Delete the file using system trash.
    Trash,
    /// Permanently delete the file.
    Delete, // (todo: confirmation).
    /// Paste all stack items into the current or specified directory
    Paste(PathBuf), // dump Stack
    /// Execute an action on the current item according [Lessfilter rules](crate::lessfilter::RulesConfig)
    Lessfilter {
        preset: Preset,
        paging: bool, // whether to feed the output to a pager
        header: When,
    },

    // Nonbindable
    // ----------------------------------
    SaveInput,
    SetHeader(Option<Text<'static>>),
    SetFooter(Option<Text<'static>>),
    Reload,
    AcceptPrompt,
    AcceptPrint,

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
    let raw_input = state.picker_ui.results.cursor_disabled || state.overlay_index().is_some();

    match a {
        Action::Custom(fa) => match fa {
            // handle nonbindable events here so that overlays don't intercept them.
            // -------------------------------------------------
            FsAction::Reload => {
                state.picker_ui.selector.revalidate();
                fs_reload(state);
                acs![]
            }
            FsAction::SaveInput => {
                let (content, index) = (
                    state.picker_ui.input.input.clone(),
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
                    state.picker_ui.header.clear();
                }
                acs![]
            }
            FsAction::SetFooter(text) => {
                if let Some(text) = text {
                    state.footer_ui.set(text);
                } else {
                    state.footer_ui.clear();
                }
                acs![]
            }

            // Actions which only trigger when not in the prompt:
            // -------------------------------------------------
            FsAction::Jump(_, c) => {
                if raw_input && let Some(c) = c {
                    acs![Action::Input(c)]
                } else {
                    acs![Action::Custom(fa)]
                }
            }
            FsAction::Parent => {
                if raw_input {
                    acs![Action::BackwardChar]
                } else if APP::in_app_pane() {
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }
            FsAction::Advance => {
                if raw_input {
                    acs![Action::ForwardChar]
                } else if APP::in_app_pane() {
                    // todo!()
                    acs![]
                } else {
                    acs![Action::Custom(fa)]
                }
            }

            //  ------------- Overlay aliases --------------
            FsAction::Stash | FsAction::Filters | FsAction::Menu if raw_input => {
                acs![fa]
            }
            FsAction::Stash => {
                acs![Action::Overlay(0)]
            }
            FsAction::Filters => {
                acs![Action::Overlay(1)]
            }
            // todo: matchmaker needs to support activating the overlay ourselves so that the activated item is aligned
            FsAction::Menu => {
                if let Some(p) = state.current_item() {
                    TEMP::set_input_bar(None, Ok(p.clone()));
                    acs![Action::Overlay(2)]
                } else if let Some(cwd) = STACK::cwd() {
                    TEMP::set_input_bar(None, Err(cwd));
                    acs![Action::Overlay(2)]
                } else {
                    acs![]
                }
            }

            // todo: support post-creation actions
            FsAction::New => {
                if let Some(cwd) = STACK::nav_cwd()
                    && state.overlay_index().is_none()
                {
                    TEMP::set_input_bar(Some(PromptKind::NewDir), Err(cwd));
                    acs![Action::Overlay(2)]
                }
                // no support for creating outside of nav
                else {
                    acs![fa]
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
                } else
                // in prompt
                if state.picker_ui.results.cursor_disabled {
                    // jump out
                    if digit > 0 {
                        enter_prompt(state, false);
                        acs![Action::Pos(digit as i32 - 1)]
                    } else {
                        // accept the prompt
                        if let Some(cwd) = STACK::cwd() {
                            // same as Accept on ::Nav
                            if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                                // print cwd
                                let s = cwd.to_string_lossy().to_string();
                                GLOBAL::db().bump(true, cwd);
                                prints!(s);
                                acs![Action::Quit(0)]
                            } else {
                                let path = cwd.inner().into();
                                let pool = GLOBAL::db();

                                TASKS::spawn(async move {
                                    let conn = unwrap!(
                                        pool.get_conn(crate::db::DbTable::dirs).await._elog()
                                    );
                                    open_wrapped(conn, None, &[path], true).await._elog();
                                });

                                acs![Action::Quit(0)]
                            }
                        } else {
                            acs![]
                        }
                    }
                } else
                // not in prompt
                if digit > 0 {
                    // accept
                    acs![
                        Action::Pos((digit - 1) as i32),
                        if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                            FsAction::AcceptPrint.into()
                        } else if GLOBAL::with_cfg(|c| c.interface.autojump_advance) {
                            FsAction::Advance.into()
                        } else {
                            Action::Accept
                        }
                    ]
                } else
                // 0 when not in prompt -> enter prompt
                {
                    enter_prompt(state, true);
                    acs![]
                }
            }
            _ => acs![fa],
        },
        _ => match a {
            Action::Up(i) => {
                TOAST::clear();

                if state.overlay_index().is_some() {
                    acs![a]
                } else if state.picker_ui.results.cursor_disabled {
                    enter_prompt(state, false);
                    acs![Action::Up(i)]
                } else if i as u32 <= state.picker_ui.results.index() {
                    acs![a]
                } else {
                    // entering the prompt
                    enter_prompt(state, true);
                    acs![]
                }
            }
            Action::Down(i) => {
                TOAST::clear();

                if state.overlay_index().is_none() && state.picker_ui.results.cursor_disabled {
                    enter_prompt(state, false);
                    acs![Action::Down(i.saturating_sub(1))]
                } else {
                    acs![a]
                }
            }
            Action::Pos(_)
                if state.overlay_index().is_none() && state.picker_ui.results.cursor_disabled =>
            {
                enter_prompt(state, false);
                acs![a]
            }

            // there's a bit of an edge case where this doesn't detect whether to be in prompt correctly for consecutive actions but for expediency we leave this as won't fix
            Action::Accept => {
                if GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                    acs![Action::Custom(FsAction::AcceptPrint), Action::Quit(0)]
                } else if state.overlay_index().is_none() && state.picker_ui.results.cursor_disabled
                {
                    acs![FsAction::AcceptPrompt]
                } else {
                    acs![Action::Accept]
                }
            }

            Action::Print(s) if s.is_empty() => {
                if !GLOBAL::with_cfg(|c| c.interface.alt_accept) {
                    acs![FsAction::AcceptPrint]
                } else if state.overlay_index().is_none() && state.picker_ui.results.cursor_disabled
                {
                    acs![FsAction::AcceptPrompt]
                } else {
                    acs![Action::Accept]
                }
            }

            Action::Reload(s)
                if s.is_empty() && STACK::with_current(|c| matches!(c, FsPane::Stream { .. })) =>
            {
                TOAST::push_msg("Cannot reload streams", false);
                acs![]
            }
            _ => acs![a],
        },
    }
}

pub fn fsaction_handler(
    a: FsAction,
    state: &mut MMState<'_, '_>,
) {
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
            STACK::push(pane);

            prepare_prompt(state);
            fs_reload(state);
        }

        FsAction::History => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            let _ = STACK::swap_history();

            prepare_prompt(state);
            fs_reload(state);
        }

        FsAction::Rg => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // let pane = FsPane::new_fd(
            //     STACK::cwd().unwrap_or_default(),
            //     FILTERS::sort(),
            //     FILTERS::visibility(),
            // );
            // STACK::push(pane);
            // todo!();

            prepare_prompt(state);
            fs_reload(state);
        }

        FsAction::Undo => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // adjust stack
            if STACK::stack_prev() {
                prepare_prompt(state);
                fs_reload(state);
            };
        }
        FsAction::Redo => {
            // save input
            let (content, index) = state.get_content_and_index();
            STACK::save_input(content, index);

            // adjust stack
            if STACK::stack_next() {
                prepare_prompt(state);
                fs_reload(state);
            };
        }

        FsAction::Jump(d, c) => {
            let path = if c == Some('\0') {
                AbsPath::new_unchecked(d)
            } else {
                let path = d.abs(__home());
                let path = AbsPath::new_unchecked(&path);

                if Some(&path) == STACK::cwd().as_ref() {
                    return;
                }

                if !path.is_dir() {
                    TOAST::push_msg(
                        vec![
                            Span::styled(d.to_string_lossy().to_string(), Color::Red),
                            Span::raw(" is not a directory!"),
                        ],
                        false,
                    );
                    return;
                }

                path
            };

            enter_dir_pane(state, path);
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
            TEMP::set_prev_dir(cwd);
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
            STASH::insert(state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                cb_vec.push(s.path.inner());
                StashItem::mv(s.path.clone())
            }));
            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Cut: ", toast_vec);
                copy_files(cb_vec, false);
            };
        }
        FsAction::Copy => {
            let mut toast_vec = vec![];
            let mut cb_vec = vec![];
            STASH::insert(state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                cb_vec.push(s.path.inner());
                StashItem::cp(s.path.clone())
            }));
            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Copied: ", toast_vec);
                copy_files(cb_vec, false);
            };
        }
        FsAction::Symlink => {
            let mut toast_vec = vec![];
            STASH::insert(state.map_selected_to_vec(|s| {
                toast_vec.push(short_display(&s.path));
                StashItem::cp(s.path.clone())
            }));
            if !toast_vec.is_empty() {
                TOAST::push(ToastStyle::Normal, "Stashed: ", toast_vec);
            };
        }
        FsAction::Backup => {
            todo!();
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
        FsAction::Delete => {
            let mut items = vec![];
            state.map_selected_to_vec(|s| {
                items.push(s.path.inner());
            });

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
        FsAction::CopyPath => {
            let paths = if !state.picker_ui.results.cursor_disabled {
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
                    TOAST::push_notice(ToastStyle::Normal, "No current directory.");
                    return;
                }
            } else {
                if !dest_base.is_absolute() {
                    TOAST::push_notice(
                        ToastStyle::Error,
                        format!("{} is not absolute.", dest_base.to_string_lossy()),
                    );
                    return;
                }
                AbsPath::new_unchecked(dest_base)
            };
            STASH::transfer_all(base, false);
        }
        FsAction::ClearStash => {
            STASH::clear_invalid_and_completed();
            TOAST::push_notice(ToastStyle::Normal, "Stack cleared");
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

                if !p_str.is_empty() {
                    let prompt = Span::styled(
                        p_str,
                        Style::default()
                            .fg(Color::Blue)
                            .add_modifier(Modifier::ITALIC),
                    );
                    state.picker_ui.input.prompt = prompt;
                } else {
                    state.picker_ui.input.reset_prompt();
                }
            } else {
                FILTERS::with_mut(|_sort, vis| {
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
                            state.picker_ui.input.prompt = Span::styled(
                                "d: ",
                                Style::default()
                                    .fg(Color::Blue)
                                    .add_modifier(Modifier::ITALIC),
                            );
                        } else if vis.files {
                            state.picker_ui.input.prompt = Span::styled(
                                "f: ",
                                Style::default()
                                    .fg(Color::Blue)
                                    .add_modifier(Modifier::ITALIC),
                            );
                        } else {
                            state.picker_ui.input.reset_prompt();
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
                    TOAST::push_msg(Span::styled("Default filters", style), true);
                } else {
                    vis.hidden = true;
                    TOAST::push_msg(Span::styled("Showing hidden", style), true);
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
        } => {
            if APP::in_app_pane() {
                // todo
                return;
            }

            if state.current_raw().is_none() && !state.picker_ui.results.cursor_disabled {
                return;
            };

            // since in Nav pane, Advance is bound to edit cursor item, it's more useful to make the action always edit the menu item.
            if matches!(preset, Preset::Edit)
                && STACK::nav_cwd().is_some()
                && state.current_raw().is_some_and(|x| x.path.is_file())
            {
                TEMP::set_that_execute_handler_should_process_cwd();
            }

            let mut template = if state
                .preview_set_payload
                .as_ref()
                .is_some_and(|s| s.is_empty())
            {
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
                    template.push_str(&format!(" | cmd /c \"set PG_LANG=toml && {pp}\" > CON"));
                    #[cfg(unix)]
                    template.push_str(&format!(" | PG_LANG=toml {pp} > /dev/tty"));
                } else {
                    wbog!(
                        "Pager path could not be decoded, please check your installation's cache directory."
                    )
                }
            }

            state.set_interrupt(Interrupt::Execute, template);
        }

        FsAction::AcceptPrompt => {
            // accepting on nav pane prompt opens the displayed directory
            if let FsPane::Nav { cwd, .. } = STACK::current() {
                let path = cwd.inner().into();
                let pool = GLOBAL::db();

                TASKS::spawn(async move {
                    let conn = unwrap!(pool.get_conn(crate::db::DbTable::dirs).await.ok());
                    open_wrapped(conn, None, &[path], true).await._elog();
                });

                state.should_quit = true;
            } else if let Some(cwd) = STACK::cwd() {
                enter_dir_pane(state, cwd);
            }
        }

        FsAction::AcceptPrint => {
            if state.picker_ui.results.cursor_disabled
                && let Some(p) = STACK::cwd()
            {
                // print cwd
                let s = p.to_string_lossy().to_string();
                GLOBAL::db().bump(true, p);
                prints!(s);
            } else {
                // if alt_accept, this was aliased from Accept, in which case we should respect no_multi_accept
                if GLOBAL::with_cfg(|c| c.interface.alt_accept && c.interface.no_multi_accept) {
                    if let Some(item) = state.current_raw() {
                        let s = item.display().to_string();
                        GLOBAL::db().bump(item.path.is_dir(), item.path.clone());
                        prints!(s);
                    }
                } else {
                    // print selected
                    state.map_selected_to_vec(|item| {
                        let s = item.display().to_string();
                        GLOBAL::db().bump(item.path.is_dir(), item.path.clone());
                        prints!(s);
                    });
                }
            }
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
    Advance, Parent, Find, Rg, History,
    Undo, Redo,
    Filters, Stash, ClearStash,
    Menu, ToggleDirs, ToggleHidden,
    Cut, Copy, CopyPath, New, NewDir,
    Symlink, Backup, Trash, Delete;

    tuples:
    AutoJump;

    defaults:
    ;
    options:
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
                            write!(f, stringify!($pathbuf))
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
                    Jump(path, _) => {
                        if path.is_empty() {
                            write!(f, "Jump(⌂)")
                        } else {
                            write!(f, "Jump({})", path.display())
                        }
                    }
                    SaveInput | SetHeader(_) | SetFooter(_) | Reload | AcceptPrompt | AcceptPrint => Ok(()), // internal
                    Lessfilter { preset, paging, header } => {
                        let mut preset = preset.to_string();
                        if *paging {
                            preset.push('|')
                        };
                        write!(f, "Lessfilter({preset})")
                    }
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
                        let path_str = data.ok_or_else(|| "Missing path for Jump")?;
                        Ok(Self::Jump(path_str.into(), None))
                    }
                    "Lessfilter" => {
                        let mut paging = false;
                        let mut preset_str = data.ok_or_else(|| "Missing preset for Lessfilter")?;

                        if let Some(stripped) = preset_str.strip_suffix('|') {
                            preset_str = stripped;
                            paging = true;
                        }

                        let preset = preset_str.to_lowercase().parse().map_err(|e| format!("Invalid preset for lessfilter: {preset_str}"))?;
                        let header = When::default();
                        Ok(Self::Lessfilter { preset, paging, header })
                    }
                    /* ------------------------------------- */

                    _ => Err(format!("Unknown action {}", s)),
                }
            }
        }
    };
}
use enum_from_str_display;
