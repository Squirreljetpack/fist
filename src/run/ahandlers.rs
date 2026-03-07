use cli_boilerplate_automation::{bring::split::split_whitespace_preserve_single_quotes, vec_};
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
        stash::{CustomStashActionActionState, STASH},
        state::{FILTERS, GLOBAL, STACK, TOAST, TlsStore, ui::global_ui},
    },
    utils::formatter::format_prompt,
};

pub fn paste_handler(
    content: String,
    state: &MMState<'_, '_>,
) -> String {
    if let Some(c) = STACK::nav_cwd()
        && !(GLOBAL::with_cfg(|c| c.interface.always_paste)
            || state.picker_ui.results.cursor_disabled
            || state.overlay_index().is_some())
    {
        STASH::transfer_all(c, false);
        String::new()
    } else {
        content
    }
}

/// sets in state: cursor (jump/disable), stashes preview, sets prompt
pub fn enter_prompt(
    state: &mut MMState<'_, '_>,
    enter: bool,
) {
    // unfortunately, dim is kinda weak/can make things brighter, but we still want some indication
    if let Some(dim) = GLOBAL::with_cfg(|c| c.interface.dim_prompt) {
        let should_dim = enter ^ !dim;

        let mods = &mut state.picker_ui.results.config.modifier;
        let border_mods = &mut state.picker_ui.results.config.border.modifier;

        if should_dim {
            *mods |= Modifier::DIM;
            *border_mods |= Modifier::DIM;
        } else {
            mods.remove(Modifier::DIM);
            border_mods.remove(Modifier::DIM);
        }
    }
    // set prompt
    if enter {
        let prompt = if let Some(cwd) = STACK::cwd() {
            let content =
                format_prompt(&GLOBAL::with_cfg(|c| c.interface.cwd_prompt.clone()), &cwd);
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

    log::debug!("entered prompt: {enter}");
}

pub fn enter_dir_pane(
    state: &mut MMState<'_, '_>,
    path: AbsPath,
) {
    // save input
    let (content, index) = state.get_content_and_index();
    STACK::save_input(content, index);
    // record
    GLOBAL::db().bump(true, path.clone());

    // apply specific settings
    if STACK::with_current(FsPane::should_cancel_input_entering_dir) {
        state.picker_ui.input.cancel();
    }

    // always clear selections
    state.picker_ui.selector.clear();
    TOAST::clear_msgs();

    // start pane
    let is_new = STACK::nav_cwd().is_none();
    let pane = FsPane::new_nav(path, FILTERS::visibility(), FILTERS::sort());
    STACK::push(pane);
    fs_reload(state, is_new);
}

pub fn fs_reload(
    state: &mut MMState<'_, '_>,
    is_new: bool,
) {
    if is_new {
        // clear worker entries
        state
            .picker_ui
            .worker
            .set_stability(STACK::with_current(FsPane::stability_threshold));
    }

    state.restart_worker();

    let injector = IndexedInjector::new_globally_indexed(state.injector());

    #[allow(warnings)]
    STACK::with_current_mut(|p| match p {
        FsPane::Search {
            filtering,
            patterns,
            input,
            ..
        } => {
            if *filtering {
                input.0 = state.picker_ui.input.input.clone();
            } else {
                let p = split_whitespace_preserve_single_quotes(&state.picker_ui.input.input);
                *patterns = if p.is_empty() && GLOBAL::with_cfg(|c| !c.rg.empty_start) {
                    vec_![""]
                } else {
                    p
                };
            };
        }
        _ => {}
    });

    STACK::populate(injector, || {});
    // stash the saved index to restore it once synced
    // This is invoked only through FsAction::Undo/Redo/Restart
    TlsStore::maybe_set(STACK::take_maybe_index());

    if is_new {
        STACK::with_current_mut(|pane| {
            if let Some(pv) = GLOBAL::with_cfg(|c| c.panes.default_visibility(pane))
                && let Some(v) = pane.vis_mut()
            {
                v.apply(pv);
            }
        });

        fs_post_reload_new(state);
    } else {
        if !state.picker_ui.results.cursor_disabled {
            state.picker_ui.results.cursor_jump(0);
        }
        state.picker_ui.selector.revalidate();
        fs_post_reload(state);
    }
}

/// Call iff the pane type changes.
///
/// 1. Set pane specific config overrides:
/// - Read the current pane's enter_prompt and default prompt values to appropriately invoke [`enter_prompt`].
///
///
/// 2. Reset transient state settings without any configuration knob
///
/// 3. Set input from pane, clear selections
pub fn fs_post_reload_new(state: &mut MMState<'_, '_>) {
    // apply pane-specific config overrides
    STACK::with_current(|pane| {
        GLOBAL::with_cfg(|c| {
            if let Some(p) = c.panes.prompt(pane) {
                state.picker_ui.input.config.prompt = p
            };

            if let Some(p) = state.preview_ui {
                p.set_layout(c.panes.preview_layout_index(pane));
            };

            if let Some(condition) = c.panes.show_preview(pane) {
                let area = state.ui_size();
                if let Some(p) = state.preview_ui.as_mut() {
                    p.config.show = condition;
                    p.reevaluate_show_condition(area, false);
                }
            }

            // this hides the preview if needed
            if let Some(enter) = c.panes.enter_prompt(pane) {
                enter_prompt(state, enter);
            } else if !state.picker_ui.results.cursor_disabled {
                state.picker_ui.input.reset_prompt();
            }

            #[cfg(feature = "mm_overrides")]
            {
                use matchmaker_partial::Apply;
                let partial = c.mm.get(pane);

                state.ui.config.apply(partial.ui.clone());
                state.picker_ui.input.config.apply(partial.input.clone());
                state
                    .picker_ui
                    .results
                    .config
                    .apply(partial.results.clone());
                state
                    .picker_ui
                    .results
                    .status_config
                    .apply(partial.status.clone());
                state
                    .preview_ui
                    .as_mut()
                    .unwrap()
                    .config
                    .apply(partial.preview.clone());
            }
        });
    });

    // Reset transient state settings without any configuration knob
    // ----
    // currently only rg supports scroll index
    // lowpri: maybe wider support
    if let Some(p) = state.preview_ui {
        p.config.scroll.index = None
    }
    state.picker_ui.results.config.right_align_last = true;

    // Set input from pane, clear selections
    // ----
    // input is nonempty only when called in [`FsAction::Undo`] and [`FsAction::Forward`].
    state
        .picker_ui
        .input
        .set(STACK::with_current(FsPane::get_input), u16::MAX);
    state.picker_ui.selector.clear();
    TOAST::clear_msgs();

    if STACK::in_app() {
        TOAST::clear();
        STASH::set_cas(CustomStashActionActionState::App);
    } else if let Some(cas) = TlsStore::get() {
        STASH::set_cas(cas);
    }

    fs_post_reload(state);
}

pub fn fs_post_reload(state: &mut MMState<'_, '_>) {
    STACK::with_current(|pane| {
        match pane {
            // we set styles in reload, not on push, because of undo/redo
            FsPane::Search {
                filtering,
                patterns,
                input,
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
                        &c.panes.search.fs_status_template
                    } else {
                        &c.panes.search.rg_status_template
                    };
                    let mut t = StatusUI::parse_template_to_status_line(base);
                    let replacement = if f { &patterns.join(" / ") } else { &input.0 };
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

                state.filtering = f;
            }
            _ => {
                // match pane {
                //     FsPane::Apps { .. } => {
                //     }
                //     _ => {}
                // }

                // restore non-rg settings
                {
                    let r = &mut state.picker_ui.results;
                    // todo: save and restore
                    r.config.horizontal_separator = Default::default();
                    r.config.stacked_columns = false;
                    r.set_status_line(None);
                }

                state.filtering = true;
                GLOBAL::send_bind(BindDirective::Unbind(Event::QueryChange.into()))
            }
        }
        log::trace!("{pane:?}");
    });
}
