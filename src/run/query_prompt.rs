//! The query prompt: its declarative definition, the prompt-mode toggle
//! ([`lock_prompt`]), the cwd-lock entry ([`enter_prompt`]), and the
//! matchmaker-mode mirror ([`prompt_mode`]).

use cba::_trace;
use matchmaker::message::BindDirective;
use ratatui::text::Line;

use crate::run::state::GLOBAL::cfg;
use crate::{
    aliases::MMState,
    run::state::{FILTERS, GLOBAL, InPrompt, STACK, STORE, ui::prompt_main_style},
    utils::formatter::format_prompt,
};

/// Declarative prompt:
/// - cursor disabled (prompt mode): the directory prompt (falls back to the
///   configured default prompt when there is no cwd);
/// - otherwise: "d: " / "f: " when visibility is dirs-only / files-only,
///   else the pane's configured prompt.
pub fn refresh_prompt(state: &mut MMState<'_>) {
    if state.picker_ui.results.cursor_disabled() {
        if let Some(cwd) = STACK::cwd() {
            let content = format_prompt(&GLOBAL::cfg().interface.cwd_prompt.clone(), &cwd);
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
            // the pane's configured prompt applied via temporary setters;
            // panes without a configured prompt restore the config default
            let (custom_prompt, custom_style) = STACK::with_current(|pane| {
                (
                    cfg().panes.prompt(pane),
                    cfg()
                        .panes
                        .prompt_style(pane)
                        .map(ratatui::style::Style::from),
                )
            });

            match (custom_prompt, custom_style) {
                (None, None) => state.picker_ui.query.set_prompt(None),
                (p, s) => {
                    let prompt = p.unwrap_or_else(|| state.picker_ui.query.config.prompt.clone());
                    let style =
                        s.unwrap_or_else(|| state.picker_ui.query.config.prompt_style.into());

                    state
                        .picker_ui
                        .query
                        .set_prompt_line(Line::styled(prompt, style));
                }
            }
        }
    }
}

/// Toggle the prompt mode (raw flag): the query bar is active while in the
/// prompt — edit-actions (left/right, Delete, paste) edit the query instead
/// of navigating, the border marks the mode, and `enter = false` also
/// restores the cursor if it was disabled. The cwd lock implies the prompt
/// mode and additionally makes actions apply to the cwd.
pub fn lock_prompt(
    state: &mut MMState<'_>,
    enter: bool,
) {
    _trace!(enter);
    // the marker tracks the raw prompt state (query bar active)
    let was_in_prompt = STORE::contains::<InPrompt>();
    if enter {
        STORE::set(InPrompt);
    } else {
        STORE::take::<InPrompt>();
    }
    // mirror only on real transitions so the mode stack stays balanced
    if was_in_prompt != enter {
        prompt_mode(enter);
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

/// Mirror the prompt state into the matchmaker mode stack so that
/// mode-tagged binds (`prompt^^...` in mm.toml) apply while the prompt is
/// active and fall back to the unconditional binds on leave.
fn prompt_mode(enter: bool) {
    let directive = if enter {
        BindDirective::PushMode("prompt".into())
    } else {
        BindDirective::PopMode("prompt".into())
    };
    GLOBAL::send_bind(directive);
}

/// Whether the query bar is active (the raw prompt state).
pub fn in_prompt() -> bool {
    STORE::contains::<InPrompt>()
}

/// Prompt entry for a "cursor-disabling pathway" (Up/Down past the ends,
/// first Accept, AutoJump(0)): enters the prompt and locks the active item
/// onto the cwd — actions then apply to the cwd. Returns `false` (and does
/// nothing) when there is no cwd to point at — Apps panes — in which case
/// the caller passes the triggering key through. Unlike [`lock_prompt`]
/// this entry also disables the results cursor: the prompt shows the cwd,
/// the selection is the locked item.
pub fn enter_prompt(state: &mut MMState<'_>) -> bool {
    if STACK::cwd().is_none() {
        return false;
    }
    if !state.picker_ui.results.cursor_disabled()
        && GLOBAL::cfg().interface.hide_preview_when_cursor_disabled
        && let Some(_p) = state.preview_ui
    {
        state.stash_preview_visibility(Some(false));
    }
    // enter the prompt mode unconditionally
    let was_in_prompt = STORE::contains::<InPrompt>();
    STORE::set(InPrompt);
    if !was_in_prompt {
        prompt_mode(true);
    }
    state.picker_ui.query.show_border = true;
    state.picker_ui.results.disable_cursor(true);
    refresh_prompt(state);
    true
}
