use matchmaker::nucleo::{Color, Modifier, Span, Style, injector::IndexedInjector};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        FsPane,
        stash::STASH,
        state::{FILTERS, GLOBAL, STACK, TEMP, TOAST},
    },
    utils::text::{ToastStyle, format_cwd_prompt},
};

pub fn paste_handler(
    content: String,
    state: &MMState<'_, '_>,
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
        STASH::transfer_all(base, false);
        String::new()
    }
}

/// read
pub fn enter_prompt(
    state: &mut MMState<'_, '_>,
    enter: bool,
) {
    // set prompt
    if enter {
        let prompt = if let Some(cwd) = STACK::cwd() {
            let content =
                format_cwd_prompt(&GLOBAL::with_cfg(|c| c.interface.cwd_prompt.clone()), &cwd);
            Span::styled(
                content,
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::ITALIC),
            )
        } else {
            let content = state.picker_ui.input.config.prompt.clone();
            Span::styled(
                content,
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::ITALIC),
            )
        };
        state.picker_ui.results.cursor_jump(0);
        state.stash_preview_visibility(Some(false));
        state.picker_ui.input.prompt = prompt;
    } else {
        state.stash_preview_visibility(None);
        state.picker_ui.input.reset_prompt();
    }
    state.picker_ui.results.cursor_disabled = enter;
}

// read the current pane's enter_prompt and default prompt values
// call after creating new pane
pub fn prepare_prompt(state: &mut MMState<'_, '_>) {
    STACK::with_current(|pane| {
        if let Some(p) = GLOBAL::with_cfg(|c| c.panes.prompt(pane)) {
            state.picker_ui.input.config.prompt = p
        };

        if GLOBAL::with_cfg(|c| c.panes.enter_prompt(pane)) {
            enter_prompt(state, true);
        } else {
            // do nothing
        }
    });

    // input is nonempty only when called in [`FsAction::Undo`] and [`FsAction::Forward`].
    state
        .picker_ui
        .input
        .set(STACK::with_current(FsPane::get_input), u16::MAX);

    // always clear selections
    state.picker_ui.selector.clear();

    if !state.picker_ui.results.cursor_disabled {
        state.picker_ui.input.reset_prompt();
    }
}

pub fn enter_dir_pane(
    state: &mut MMState<'_, '_>,
    path: AbsPath,
) {
    // save input
    let (content, index) = state.get_content_and_index();
    STACK::save_input(content, index);

    if STACK::with_current(FsPane::should_cancel_input_entering_dir) {
        state.picker_ui.input.cancel();
    }

    TOAST::clear_msgs();
    // record
    GLOBAL::db().bump(true, path.clone());

    // pane
    let pane = FsPane::new_nav(path, FILTERS::visibility(), FILTERS::sort());

    // set the prompt marker
    if let Some(p) = GLOBAL::with_cfg(|c| c.panes.prompt(&pane)) {
        state.picker_ui.input.config.prompt = p
    };

    // exit prompt
    enter_prompt(state, false);
    // always clear selections
    state.picker_ui.selector.clear();

    STACK::push(pane);
    fs_reload(state);
}

// on new pane
pub fn fs_reload(state: &mut MMState<'_, '_>) {
    state.picker_ui.worker.restart(false);
    state
        .picker_ui
        .worker
        .set_stability(STACK::with_current(FsPane::stability_multiplier));
    let injector = IndexedInjector::new_globally_indexed(state.injector());
    STACK::populate(injector, || {});

    state.picker_ui.results.cursor_jump(0);
    // stash the saved index to restore it once synced
    // This is invoked only through FsAction::Undo/Redo/Restart
    if let Some(index) = STACK::take_maybe_index() {
        TEMP::set_stashed_index(index);
    }
}
