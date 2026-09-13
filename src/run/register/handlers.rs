//! Event-handler plumbing: `sync_handler` (post-reload state recovery),
//! `query_handler` (rg query changes), and `paste_handler` (paste
//! dispatch or query insertion).

use fist_types::When;
use matchmaker::message::Event;

use crate::{
    aliases::MMState,
    run::{
        pane::FsPane,
        query_prompt,
        queue::{QUEUE, QueueSelector, SelectorResult},
        reload::fs_reload,
        selection,
        state::{GLOBAL, InPrompt, LockPromptSetting, STACK, STORE, TOAST, ToastStyle},
    },
};

/// Rehydrates the selections that [`crate::run::reload::fs_reload`]
/// snapshotted as path hashes once the fresh listing has landed.
pub fn sync_handler(
    state: &mut MMState<'_>,
    _: &Event,
) {
    // reload saved state
    if let Some(seek) = STORE::take()
        && let Some(i) = state
            .picker_ui
            .worker
            .matched_results()
            .position(|x| x.path == seek)
    {
        state.picker_ui.results.cursor_jump(i as u32);
        STORE::take::<u32>();
    } else
    // this part is exclusive to [`FsAction::Undo`], Forward and watcher reload.
    if let Some(index) = STORE::take() {
        state.picker_ui.results.cursor_jump(index);
    };

    // peek: only refill once the pane has finished populating
    let ready = STORE::with::<selection::PendingSelections, _>(|pending| {
        !pending.0.is_empty() && STACK::with_current(FsPane::is_complete)
    })
    .unwrap_or(false);
    if !ready {
        return;
    }

    if let Some(selection::PendingSelections(hashes)) =
        STORE::take::<selection::PendingSelections>()
    {
        let items = state.picker_ui.worker.nucleo.items();
        let indices = selection::rehydrate(&hashes, items.iter());
        state.picker_ui.clear_selections();
        state.picker_ui.selector.extend(indices);
    }
}

pub fn query_handler(
    state: &mut MMState<'_>,
    _: &Event,
) {
    if STORE::get::<LockPromptSetting>() == Some(LockPromptSetting(When::Auto)) {
        let is_empty = state.picker_ui.query.input().is_empty();
        if is_empty {
            if query_prompt::in_prompt() {
                query_prompt::lock_prompt(state, false);
            }
        } else if !query_prompt::in_prompt() {
            query_prompt::lock_prompt(state, true);
        }
    }

    let should_reload = STACK::with_current(|pane| {
        matches!(
            pane,
            FsPane::Search {
                filtering: false,
                ..
            }
        )
    });

    if should_reload {
        fs_reload(state, false, false);
    }
}

/// Paste dispatch: with a nav cwd and no prompt/overlay active, the pasted
/// content is treated as a queue selection; otherwise it inserts into the
/// query.
pub fn paste_handler(
    content: String,
    state: &MMState<'_>,
) -> String {
    if let Some(c) = STACK::nav_cwd()
        && !(GLOBAL::cfg().interface.always_paste
            // paste-inside-the-prompt: while the prompt mode is on (raw
            // marker — set by [`crate::run::query_prompt::lock_prompt`] and
            // [`crate::run::query_prompt::enter_prompt`]), paste inserts
            // into the query
            || STORE::contains::<InPrompt>()
            || state.overlay_index().is_some())
    {
        match QUEUE::select(&QueueSelector::Builtins, Some(&c)) {
            SelectorResult::Ready(indices) => QUEUE::dispatch(indices, Some(c)),
            SelectorResult::MissingDestination => TOAST::notice(
                ToastStyle::Error,
                "Missing destination for the queued items.",
            ),
            SelectorResult::NoItems => TOAST::msg("No items queued.", true),
        }
        String::new()
    } else {
        content
    }
}
