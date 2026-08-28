use cba::{_trace, bring::split::split_whitespace_preserve_single_quotes};
use fist_types::{
    filters::{SortOrder, Visibility},
    git::in_git_repo,
};
use matchmaker::{
    acs,
    config::StringOrInt,
    message::{BindDirective, Event},
    ui::StatusUI,
};

use crate::run::state::GLOBAL::db;
use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        FsAction, FsPane, query_prompt, selection,
        state::{FILTERS, GLOBAL, STACK, STORE, TOAST, sort, ui::global_ui},
    },
};

pub fn enter_dir_pane(
    state: &mut MMState<'_>,
    path: AbsPath,
) {
    // save input
    let (content, index) = state.get_content_and_index();
    STACK::save_input(content, index);
    // record
    db().bump_path(true, path.clone());

    // apply specific settings
    // cancel the query when leaving a pane that wants it — the input was
    // saved above, so Undo/Back restores it for the dir we're leaving
    if STACK::with_current(FsPane::should_cancel_input_entering_dir) {
        state.picker_ui.query.clear();
    }

    // always clear selections
    state.picker_ui.clear_selections();
    TOAST::clear_msgs();

    // start pane
    let old = STACK::nav_cwd();
    let is_new = old.is_none();
    // enter_dir_pane is the only fs_reload caller that sets dir_changed = true.
    // is_new && dir_changed is a reserved combination for history pane
    let dir_changed = if is_new {
        false
    } else {
        old.as_ref().is_some_and(|o| o != &path)
    };

    // apply smart git visibility on entering a git repo
    // default_vis is_some is handled separately in fs_reload
    let mut vis = FILTERS::visibility();
    if GLOBAL::cfg().panes.nav.default_visibility.is_none() {
        match (
            in_git_repo(old.map(|x| x.inner())),
            in_git_repo(Some(path.inner())),
        ) {
            (false, true) => {
                vis.hidden = true;
                vis.ignore = true;
            }

            (true, false) => {
                vis.hidden = false;
                vis.ignore = false;
            }

            _ => {}
        }
    }

    let pane = FsPane::new_nav(path, vis, Default::default()).set_initial_sort();
    STACK::push(pane);
    fs_reload(state, is_new, dir_changed);
}

pub fn fs_reload(
    state: &mut MMState<'_>,
    is_new: bool,
    // whether the reload re-reads a different directory than
    // the pane currently shows. is_new || dir_changed gates the selection refill and dir-size clear.
    // is_new && dir_changed (Undo/Redo): don't apply default visibility/sort.
    // dir_changed matters only for `enter_dir` and `undo/redo`
    dir_changed: bool,
) {
    // apply vis/sort changes
    if is_new && !dir_changed {
        STACK::with_current_mut(|pane| {
            let c = GLOBAL::cfg();
            // apply on non-initial new pane: update visibility
            if let Some(mut dv) = STORE::get::<Visibility>() {
                let pv = c.panes.default_visibility(pane).unwrap_or_default();

                // behaves as if initial (fd) cmd was specified without visibility modifiers
                if let Some(v) = pane.vis_mut() {
                    dv.apply(pv);
                    *v = dv;
                    STORE::take::<Visibility>();
                }
            } else if let Some(pv) = c.panes.default_visibility(pane)
                && let Some(v) = pane.vis_mut()
            {
                v.apply(pv);
            }
        });
    }

    // snapshot hashes of the selected paths
    // before the worker restart wipes the current listing. Refill only if the
    // reload re-reads the same directory.
    {
        let refill = GLOBAL::cfg().fs.refill_selections_after_reload && !is_new && !dir_changed;
        if refill && !state.picker_ui.selector.is_empty() {
            let hashes: Vec<u64> = state
                .picker_ui
                .selector
                .iter()
                .filter_map(|&idx| state.picker_ui.worker.get_by_idx(idx))
                .map(|item| selection::hash_path(&item.path))
                .collect();
            STORE::set(selection::PendingSelections(hashes));
        } else {
            STORE::take::<selection::PendingSelections>();
        }
        state.picker_ui.clear_selections();
    }

    sort::set_sort_from_pane(state, false);
    if sort::get_sort().order == SortOrder::size && (is_new || dir_changed) {
        let preserve = GLOBAL::cfg().interface.preserve_size_cache
            && STACK::cwd()
                .as_ref()
                .is_some_and(|p| sort::dir_size().get_path(p.inner()).is_some());
        if !preserve {
            sort::clear_dir_sizes();
        }
    }
    state.worker_restart();

    let injector = state.injector();

    // if !is_new, update state from UI
    // if new, update UI from state: the creator needs ensure the state is correct on creation.
    // (post_reload_new will match UI to state).
    STACK::with_current_mut(|p| match p {
        // (always) inform search pane from query
        FsPane::Search {
            filtering,
            patterns,
            input,
            is_initial,
            ..
        } => {
            if !is_new && !is_initial.take() {
                if *filtering {
                    input.0 = state.picker_ui.query.input(); // input is saved anyway
                } else {
                    *patterns =
                        split_whitespace_preserve_single_quotes(&state.picker_ui.query.input());
                };
            }
        }
        _ => {}
    });

    STACK::populate(injector, || {});

    // --- some post-reload stuff init doesn't need to go through
    // stash the saved index to restore it once synced
    // The index is only saved through FsAction::Undo/Redo/Restart, see [`STACK::save_input`]
    if let Some(i) = STACK::take_maybe_index() {
        STORE::set(i);
    }
    if !state.picker_ui.results.cursor_disabled() {
        state.picker_ui.results.cursor_jump(0);
    }

    if is_new {
        fs_post_reload_new(state); // called by init
    } else {
        // selections can't be revalidated across a worker restart — clear them
        fs_post_reload(state);
    }
}

/// Call iff the pane type changes.
///
/// 1. Set pane specific config overrides:
/// - Read the current pane's lock_prompt and default prompt values to appropriately invoke [`lock_prompt`].
/// 2. Reset transient state settings without any configuration knob
/// 3. Set input from pane, clear selections
pub fn fs_post_reload_new(state: &mut MMState<'_>) {
    // apply pane-specific config overrides
    STACK::with_current(|pane| {
        let c = GLOBAL::cfg();
        if let Some(p) = state.preview_ui {
            p.set_layout(c.panes.preview_layout_index(pane));
        };

        if let Some(condition) = c.panes.show_preview(pane) {
            let area = state.ui_size();
            if let Some(p) = state.preview_ui.as_mut() {
                p.config.show = condition;
                p.reevaluate_show_condition(area, true);
            }
        }

        if let Some(enter) = c.panes.locks_prompt(pane) {
            // this hides the preview if needed
            query_prompt::lock_prompt(state, enter);
        } else {
            query_prompt::refresh_prompt(state);
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

    // Reset transient state settings without any configuration knob
    // ----
    // currently only rg supports scroll index
    // lowpri: maybe wider support
    if let Some(p) = state.preview_ui {
        p.config.initial.index = None
    }
    state.picker_ui.results.config.right_align_last = true;

    // Set input from pane, clear selections
    // ----
    // input is nonempty only when called in [`FsAction::Undo`] and [`FsAction::Forward`].

    // if new, update ui from rg state
    // post_reload will set the styling
    state
        .picker_ui
        .query
        .set(STACK::with_current(FsPane::get_input), u16::MAX);
    state.picker_ui.clear_selections();
    TOAST::clear_msgs();

    if STACK::in_app() {
        TOAST::clear();
    }

    fs_post_reload(state);
}

pub fn fs_post_reload(state: &mut MMState<'_>) {
    STACK::with_current(|pane| {
        match pane {
            // we set styles in reload, not on push, because of undo/redo
            FsPane::Search {
                filtering,
                patterns,
                input,
                one_line,
                ..
            } => {
                let f = *filtering;
                if let Some(p) = state.preview_ui {
                    p.config.initial.index = Some(StringOrInt::String("3".to_string()))
                }
                let r = &mut state.picker_ui.results;
                let s = &mut state.picker_ui.status;
                let mm = &global_ui().matchmaker;
                r.config.right_align_last = false;

                if !*one_line {
                    // todo: where to add a place to configure this? pane/ui/other?
                    r.config.separator = mm.horizontal_separator;
                    r.config.stacked_columns = true;
                } else {
                    r.config.separator = Default::default();
                    r.config.stacked_columns = false;
                }

                // set status
                let status = {
                    let c = GLOBAL::cfg();
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
                };
                s.set(Some(status));
                s.status_config.show = true;

                if f {
                    GLOBAL::send_bind(BindDirective::Unbind(Event::QueryChange.into()));
                } else {
                    GLOBAL::send_bind(BindDirective::Bind(
                        Event::QueryChange.into(),
                        acs![FsAction::Reload],
                    ));
                }

                state.picker_ui.filtering = f;
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
                    r.config.separator = Default::default();
                    r.config.stacked_columns = false;

                    state.picker_ui.status.set(None);
                }

                state.picker_ui.filtering = true;
                GLOBAL::send_bind(BindDirective::Unbind(Event::QueryChange.into()))
            }
        }
        _trace!(pane);
    });
}
