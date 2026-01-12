use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
};

use crate::utils::file_size;

#[derive(Default, Debug, Clone)]
pub struct StashItemStatus {
    pub state: AtomicStashItemState,
    pub progress: Arc<AtomicU8>,
    /// bytes
    pub size: Arc<AtomicU64>,
}

impl StashItemStatus {
    pub fn new(path: &Path) -> Self {
        let size = Arc::new(AtomicU64::new(file_size(path)));
        Self {
            state: Default::default(),
            progress: Default::default(),
            size,
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum StashItemState {
    Pending = 0,
    PendingErr = 1,
    Started = 2,
    CompleteOk = 3,
    CompleteErr = 4,
}

#[derive(Default, Debug, Clone)]
pub struct AtomicStashItemState {
    state: Arc<AtomicU8>,
}

impl AtomicStashItemState {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(0)),
        }
    }

    #[inline]
    pub fn load(&self) -> StashItemState {
        Self::decode(self.state.load(Ordering::Acquire))
    }

    #[inline]
    pub fn store(
        &self,
        value: StashItemState,
    ) {
        self.state.store(value as u8, Ordering::Release);
    }

    #[inline]
    pub fn compare_exchange(
        &self,
        current: StashItemState,
        new: StashItemState,
    ) -> Result<StashItemState, StashItemState> {
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
            StashItemState::CompleteOk | StashItemState::CompleteErr
        )
    }

    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self.load(), StashItemState::Pending)
    }

    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(
            self.load(),
            StashItemState::PendingErr | StashItemState::CompleteErr
        )
    }

    #[inline(always)]
    fn decode(v: u8) -> StashItemState {
        match v {
            0 => StashItemState::Pending,
            1 => StashItemState::PendingErr,
            2 => StashItemState::Started,
            3 => StashItemState::CompleteOk,
            _ => StashItemState::CompleteErr,
        }
    }
}
