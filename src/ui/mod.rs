#![allow(unused_variables)]
pub mod components;
pub mod confirm_overlay;
pub mod empty_overlay;
pub mod input;
pub mod menu_overlay;
pub mod mount_overlay;
pub mod options_overlay;
pub mod prompt_overlay;
pub mod queue_overlay;

/// Registration order of overlays passed to `PickOptions::overlay` in `crate::run::start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum OverlayId {
    Queue = 0,
    App = 1,
    Options = 2,
    Confirm = 3,
    Menu = 4,
    Mount = 5,
    Empty = 6,
}

impl OverlayId {
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// Ticker rate (Hz) forced while an overlay that needs live updates is open.
/// Set on enable and unset on disable via `BindDirective::OverrideTickrate`.
pub const OVERLAY_TICK_RATE: u8 = 20;
