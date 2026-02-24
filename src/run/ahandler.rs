use matchmaker::{
    acs,
    message::{BindDirective, Event},
    nucleo::{Color, Modifier, Span, Style, injector::IndexedInjector},
    ui::StatusUI,
};

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        FsAction, FsPane,
        stash::STASH,
        state::{FILTERS, GLOBAL, STACK, TEMP, TOAST, ui::global_ui},
    },
    utils::{string::format_cwd_prompt, text::ToastStyle},
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
    // set default prompt/enter prompt
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

    // currently only rg supports scroll index
    // lowpri: maybe wider support
    if let Some(p) = state.preview_ui {
        p.config.scroll.index = None
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
        .set_stability(STACK::with_current(FsPane::stability_threshold));
    let injector = IndexedInjector::new_globally_indexed(state.injector());

    STACK::with_previous(|p, same| match p {
        FsPane::Fd { .. } => {
            if !same && GLOBAL::with_cfg(|c| c.panes.fd.on_leave_unset_dirs_only) {
                FILTERS::with_vis_mut(|v| v.dirs = false);
            }
        }
        FsPane::Rg { .. } => {
            if same {
                return;
            }
            GLOBAL::with_cfg(|_c| {
                let r = &mut state.picker_ui.results;

                // todo: save and restore
                r.config.horizontal_separator = Default::default();
                r.config.stacked_columns = false;
                r.set_status_line(None);
            })
        }
        _ => {}
    });

    STACK::with_current_mut(|pane| {
        GLOBAL::with_cfg(|cfg| {
            if let Some(x) = cfg.panes.preview_show(pane) {
                state.preview_ui.as_mut().map(|p| p.show(x));
            }
            if let Some(x) = cfg.panes.prompt(pane) {
                state.picker_ui.input.config.prompt = x;
                if let Some(p) = state.preview_ui {
                    p.set_layout(cfg.panes.preview_layout_index(pane));
                };
            }
        });
        state.picker_ui.results.config.right_align_last = true;

        match pane {
            FsPane::Rg {
                filtering,
                patterns,
                input,
                pattern_index,
                no_heading,
                ..
            } => {
                let f = *filtering;
                if let Some(p) = state.preview_ui {
                    p.config.scroll.index = Some("3".into())
                }
                let r = &mut state.picker_ui.results;
                let mm = &global_ui().matchmaker;
                r.config.right_align_last = false;

                if !*no_heading {
                    // todo: where to add a place to configure this? pane/ui/other?
                    r.config.horizontal_separator = mm.horizontal_separator;
                    r.config.stacked_columns = true;
                    r.status_config.show = true;
                } else {
                    r.config.horizontal_separator = Default::default();
                    r.config.stacked_columns = false;
                    r.set_status_line(None);
                }

                // set status
                // todo: more style flexibility in status
                let status = GLOBAL::with_cfg(|c| {
                    let base = if f {
                        &c.panes.rg.fs_status_template
                    } else {
                        &c.panes.rg.rg_status_template
                    };
                    let mut t = StatusUI::parse_template_to_status_line(base);
                    let replacement = if f { &patterns.join(" / ") } else { &input.0 }; // todo: lowpri: styling
                    for s in t.spans.iter_mut() {
                        s.content = s.content.replace("{}", replacement).into();
                    }
                    t
                });
                r.set_status_line(Some(status));

                if f {
                    GLOBAL::send_bind(BindDirective::Unbind(Event::QueryChange.into()));
                } else {
                    GLOBAL::send_bind(BindDirective::Bind(
                        Event::QueryChange.into(),
                        acs![FsAction::Reload],
                    ));
                }

                if f {
                    input.0 = state.picker_ui.input.input.clone()
                } else {
                    patterns[*pattern_index] = state.picker_ui.input.input.clone()
                }

                state.filtering = f;
            }
            _ => {
                state.filtering = true;
                GLOBAL::send_bind(BindDirective::Unbind(Event::QueryChange.into()))
            }
        }
    });

    STACK::populate(injector, || {});

    state.picker_ui.results.cursor_jump(0);
    // stash the saved index to restore it once synced
    // This is invoked only through FsAction::Undo/Redo/Restart
    if let Some(index) = STACK::take_maybe_index() {
        TEMP::set_stashed_index(index);
    }
}
