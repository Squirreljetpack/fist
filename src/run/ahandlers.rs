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
use ratatui::text::Line;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        FsAction, FsPane,
        queue::QUEUE,
        selection,
        state::{
            FILTERS, GLOBAL, HideMetadata, InPrompt, STACK, STORE, TOAST, sort,
            ui::{global_ui, prompt_main_style},
        },
    },
    utils::formatter::format_prompt,
};

pub fn paste_handler(
    content: String,
    state: &MMState<'_,>,
) -> String {
    if let Some(c) = STACK::nav_cwd()
        && !(GLOBAL::with_cfg(|c| c.interface.always_paste)
            // paste-inside-the-prompt: while the prompt mode is on (raw
            // marker — set by lock_prompt under prompt_locking, by
            // enter_prompt always), paste inserts into the query
            || STORE::contains::<InPrompt>()
            || state.overlay_index().is_some())
    {
        QUEUE::execute_all_impl(c, false, None);
        String::new()
    } else {
        content
    }
}

/// Declarative prompt:
/// - cursor disabled (prompt mode): the directory prompt (falls back to the
///   configured default prompt when there is no cwd);
/// - otherwise: "d: " / "f: " when visibility is dirs-only / files-only,
///   else the pane's configured prompt.
pub fn refresh_prompt(state: &mut MMState<'_,>) {
    if state.picker_ui.results.cursor_disabled() {
        if let Some(cwd) = STACK::cwd() {
            let content =
                format_prompt(&GLOBAL::with_cfg(|c| c.interface.cwd_prompt.clone()), &cwd);
            state
                .picker_ui
                .query
                .set_prompt_line(Line::styled(content, prompt_main_style()));
        } else {
            state.picker_ui.query.set_prompt(None);
        };
    } else {
        let vis = FILTERS::visibility();
        if vis.dirs && !vis.files {
            state
                .picker_ui
                .query
                .set_prompt_line(Line::styled("d: ", prompt_main_style()));
        } else if vis.files && !vis.dirs {
            state
                .picker_ui
                .query
                .set_prompt_line(Line::styled("f: ", prompt_main_style()));
        } else {
            state.picker_ui.query.set_prompt(None); // restore stored prompt
        }
    }
}

/// Toggle the prompt mode (raw flag): the query bar is active while in the
/// prompt — edit-actions (left/right, Delete, paste) edit the query instead
/// of navigating, the border marks the mode, and `enter = false` also
/// restores the cursor if it was disabled. Entering is gated on
/// `interface.prompt_locking` — with locking off, `enter = true` is a no-op
/// (the only way into the prompt is the cwd lock, [`enter_prompt`]) —
/// leaving is never gated. The cwd lock implies the prompt mode and
/// additionally makes actions apply to the cwd.
pub fn lock_prompt(
    state: &mut MMState<'_,>,
    enter: bool,
) {
    if enter && !GLOBAL::with_cfg(|c| c.interface.prompt_locking) {
        return;
    }
    _trace!(enter);
    // the marker tracks the raw prompt state (query bar active)
    if enter {
        STORE::set(InPrompt);
    } else {
        STORE::take::<InPrompt>();
    }
    // the query bar border is the prompt-mode indicator: shown only while
    // in the prompt, hidden otherwise
    state.picker_ui.query.show_border = enter;

    if !enter {
        state.stash_preview_visibility(None);
        // leaving the prompt restores the cursor (the caller may still move
        // it afterwards)
        if state.picker_ui.results.cursor_disabled() {
            state.picker_ui.results.disable_cursor(false);
        }
    }
    refresh_prompt(state);
}

/// Prompt entry for a "cursor-disabling pathway" (Up/Down past the ends,
/// first Accept, AutoJump(0)): enters the prompt and locks the active item
/// onto the cwd — actions then apply to the cwd. Returns `false` (and does
/// nothing) when there is no cwd to point at — Apps panes — in which case
/// the caller passes the triggering key through.
///
/// Reimplements lock_prompt's entry branch rather than deferring to it,
/// because lock_prompt gates entry on `interface.prompt_locking` and this
/// is the ungated entry.
pub fn enter_prompt(state: &mut MMState<'_,>) -> bool {
    if STACK::cwd().is_none() {
        return false;
    }
    if !state.picker_ui.results.cursor_disabled()
        && GLOBAL::with_cfg(|c| c.interface.hide_preview_when_cursor_disabled)
        && let Some(p) = state.preview_ui
    {
        state.stash_preview_visibility(Some(false));
    }
    // enter the prompt mode unconditionally
    STORE::set(InPrompt);
    state.picker_ui.query.show_border = true;
    state.picker_ui.results.disable_cursor(true);
    refresh_prompt(state);
    true
}

pub fn enter_dir_pane(
    state: &mut MMState<'_,>,
    path: AbsPath,
) {
    // save input
    let (content, index) = state.get_content_and_index();
    STACK::save_input(content, index);
    // record
    GLOBAL::db().bump_path(true, path.clone());

    // apply specific settings
    // cancel the query when leaving a pane that wants it — the input was
    // saved above, so Undo/Back restores it for the dir we're leaving
    if STACK::with_current(FsPane::should_cancel_input_entering_dir) {
        state.picker_ui.query.clear();
    }

    // always clear selections
    state.picker_ui.selector.clear();
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
    if GLOBAL::with_cfg(|c| c.panes.nav.default_visibility.is_none()) {
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

    let pane = FsPane::new_nav(path, vis, sort::get_sort().order);
    STACK::push(pane);
    fs_reload(state, is_new, dir_changed);
}

pub fn fs_reload(
    state: &mut MMState<'_,>,
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
            GLOBAL::with_cfg(|c| {
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
                let new_sort = c.panes.default_sort(pane);
                match pane {
                    FsPane::Custom { sort, vis, .. }
                    | FsPane::Find { sort, vis, .. }
                    | FsPane::Search { sort, vis, .. }
                    | FsPane::Nav { sort, vis, .. } => {
                        if c.panes.settings.apply_default_sort
                            && let Some(new_sort) = new_sort
                        {
                            *sort = new_sort;
                        }

                        FILTERS::set(*vis);
                    }
                    FsPane::Files { .. }
                    | FsPane::Folders { .. }
                    | FsPane::Apps { .. }
                    | FsPane::Stash { .. } => {
                        // logically we should add configurable default but i don't think anything besides frecency is desirable [for the default]
                    }
                }
            })
        });
    }

    // snapshot hashes of the selected paths
    // before the worker restart wipes the current listing. Refill only if the
    // reload re-reads the same directory.
    {
        let refill =
            GLOBAL::with_cfg(|c| c.fs.refill_selections_after_reload) && !is_new && !dir_changed;
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
        state.picker_ui.selector.clear();
    }

    // apply the pane's sort
    sort::set_sort_from_pane(state);
    if sort::get_sort().order == SortOrder::size // don't clear dirsize cache for auto-reloads (todo: lowpri: configurable)
    && (is_new || dir_changed)
    {
        sort::clear_dir_sizes();
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
pub fn fs_post_reload_new(state: &mut MMState<'_,>) {
    // apply pane-specific config overrides
    STACK::with_current(|pane| {
        GLOBAL::with_cfg(|c| {
            if let Some(p) = c.panes.prompt(pane) {
                state.picker_ui.query.config.prompt = p
            };

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
                lock_prompt(state, enter);
            } else {
                refresh_prompt(state);
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

    // a pane whose sort matches its configured default override (startup
    // pane, pane switch with apply_default_sort, undo/redo) hides the
    // metadata column until the user explicitly re-sorts (ReSort takes it)
    if STACK::with_current(|pane| {
        GLOBAL::with_cfg(|c| c.panes.default_sort(pane).is_some_and(|d| pane.sort() == d))
    }) {
        STORE::set(HideMetadata);
    }

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
    state.picker_ui.selector.clear();
    TOAST::clear_msgs();

    if STACK::in_app() {
        TOAST::clear();
    }

    fs_post_reload(state);
}

pub fn fs_post_reload(state: &mut MMState<'_,>) {
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
