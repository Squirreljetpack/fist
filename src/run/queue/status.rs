use std::{
    path::Path,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
};

use crate::find::metadata::file_size;

#[derive(Default, Debug, Clone)]
pub struct QueueItemStatus {
    pub state: AtomicQueueItemState,
    pub progress: Arc<AtomicU8>,
    /// bytes
    pub size: Arc<AtomicU64>,
}

impl QueueItemStatus {
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
pub enum QueueItemState {
    Pending = 0,
    PendingErr = 1,
    Started = 2,
    CompleteOk = 3,
    CompleteErr = 4,
}

bitflags::bitflags! {
    /// Queue item states. Mirrors [`QueueItemState`] exactly.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct QueueItems: u8 {
        const Pending = 1 << 0;
        const PendingErr = 1 << 1;
        const Started = 1 << 2;
        const CompleteOk = 1 << 3;
        const CompleteErr = 1 << 4;
    }
}

impl QueueItemState {
    /// The clearing bitflag for this state.
    pub fn to_bitflag(&self) -> QueueItems {
        match self {
            Self::Pending => QueueItems::Pending,
            Self::PendingErr => QueueItems::PendingErr,
            Self::Started => QueueItems::Started,
            Self::CompleteOk => QueueItems::CompleteOk,
            Self::CompleteErr => QueueItems::CompleteErr,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct AtomicQueueItemState {
    state: Arc<AtomicU8>,
}

impl AtomicQueueItemState {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(0)),
        }
    }

    #[inline]
    pub fn load(&self) -> QueueItemState {
        Self::decode(self.state.load(Ordering::Acquire))
    }

    #[inline]
    pub fn store(
        &self,
        value: QueueItemState,
    ) {
        self.state.store(value as u8, Ordering::Release);
    }

    #[inline]
    pub fn compare_exchange(
        &self,
        current: QueueItemState,
        new: QueueItemState,
    ) -> Result<QueueItemState, QueueItemState> {
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
    pub fn is_started(&self) -> bool {
        matches!(self.load(), QueueItemState::Started)
    }

    pub fn is_complete(&self) -> bool {
        matches!(
            self.load(),
            QueueItemState::CompleteOk | QueueItemState::CompleteErr
        )
    }

    #[inline]
    pub fn is_pending(&self) -> bool {
        matches!(self.load(), QueueItemState::Pending)
    }

    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(
            self.load(),
            QueueItemState::PendingErr | QueueItemState::CompleteErr
        )
    }

    #[inline(always)]
    fn decode(v: u8) -> QueueItemState {
        match v {
            0 => QueueItemState::Pending,
            1 => QueueItemState::PendingErr,
            2 => QueueItemState::Started,
            3 => QueueItemState::CompleteOk,
            _ => QueueItemState::CompleteErr,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The clearing bitflag of a state is its discriminant shifted into a bit
    /// position (identity mapping), so the two cannot drift apart.
    #[test]
    fn bitflag_round_trip() {
        for state in [
            QueueItemState::Pending,
            QueueItemState::PendingErr,
            QueueItemState::Started,
            QueueItemState::CompleteOk,
            QueueItemState::CompleteErr,
        ] {
            assert_eq!(state.to_bitflag().bits(), 1u8 << (state as u8));
        }

        let flags = [
            QueueItems::Pending,
            QueueItems::PendingErr,
            QueueItems::Started,
            QueueItems::CompleteOk,
            QueueItems::CompleteErr,
        ];
        for (i, flag) in flags.iter().enumerate() {
            assert_eq!(flag.bits(), 1u8 << i);
        }
    }
}
