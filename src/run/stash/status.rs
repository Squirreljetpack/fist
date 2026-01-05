use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

use cli_boilerplate_automation::text::StrExt;
use ratatui::{
    style::{Color, Style},
    text::Span,
};

use crate::{
    ui::stash_overlay::StackConfig,
    utils::{file_size, format_size},
};

#[derive(Default, Debug, Clone)]
pub struct StackItemStatus {
    pub state: AtomicStackItemState,
    pub progress: Arc<AtomicU8>,
    /// bytes
    pub size: Arc<AtomicU64>,
}

impl StackItemStatus {
    pub fn new(path: &Path) -> Self {
        let size = Arc::new(AtomicU64::new(file_size(path)));
        Self {
            state: Default::default(),
            progress: Default::default(),
            size,
        }
    }
}

impl StackItemStatus {
    pub fn render(
        &self,
        cfg: &StackConfig,
    ) -> Span<'static> {
        let size = self.size.load(Ordering::Relaxed);
        let progress = self.progress.load(Ordering::Relaxed);
        let state = self.state.load();

        let bar_text = if matches!(state, StackItemState::Started) {
            let filled_width = progress * cfg.bar_width.0 / 255;
            let empty_width = cfg.bar_width.0 - filled_width;
            let progress_text = format!("{:.00}%", (progress as f32 / 255.0) * 100.0);

            format!(
                "[{}{} {}]",
                "█".repeat(filled_width as usize),
                "░".repeat(empty_width as usize),
                progress_text
            )
        } else {
            format_size(size)
        }
        .pad(1, 1);

        let style = match state {
            StackItemState::Pending => Style::default(),
            StackItemState::Started => Style::default().fg(Color::Cyan),
            StackItemState::CompleteOk => Style::default().fg(Color::Green),
            StackItemState::PendingErr | StackItemState::CompleteErr => {
                Style::default().fg(Color::Red)
            }
        };

        Span::styled(bar_text, style)
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum StackItemState {
    Pending = 0,
    PendingErr = 1,
    Started = 2,
    CompleteOk = 3,
    CompleteErr = 4,
}

#[derive(Default, Debug, Clone)]
pub struct AtomicStackItemState {
    state: Arc<AtomicU8>,
}

impl AtomicStackItemState {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(0)),
        }
    }

    #[inline]
    pub fn load(&self) -> StackItemState {
        Self::decode(self.state.load(Ordering::Acquire))
    }

    #[inline]
    pub fn store(
        &self,
        value: StackItemState,
    ) {
        self.state.store(value as u8, Ordering::Release);
    }

    #[inline]
    pub fn compare_exchange(
        &self,
        current: StackItemState,
        new: StackItemState,
    ) -> Result<StackItemState, StackItemState> {
        self.state
            .compare_exchange(
                current as u8,
                new as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(Self::decode)
            .map_err(Self::decode)
    }

    #[inline]
    pub fn is_complete(&self) -> bool {
        matches!(
            self.load(),
            StackItemState::CompleteOk | StackItemState::CompleteErr
        )
    }

    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(
            self.load(),
            StackItemState::PendingErr | StackItemState::CompleteErr
        )
    }

    #[inline(always)]
    fn decode(v: u8) -> StackItemState {
        match v {
            0 => StackItemState::Pending,
            1 => StackItemState::PendingErr,
            2 => StackItemState::Started,
            3 => StackItemState::CompleteOk,
            _ => StackItemState::CompleteErr,
        }
    }
}
