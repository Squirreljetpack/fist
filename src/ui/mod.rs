#![allow(unused_variables, unused_mut, unreachable_code)]
pub mod confirm_overlay;
pub mod input;
pub mod menu_overlay;
mod menu_overlay_impl;
pub mod options_overlay;
pub mod prompt_overlay;
pub mod queue_overlay;

/// Ticker rate (Hz) forced while an overlay that needs live updates is open.
/// Set on enable and unset on disable via `BindDirective::OverrideTickrate`.
pub const OVERLAY_TICK_RATE: u8 = 20;
