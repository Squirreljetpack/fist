use std::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TaskState {
    Pending = 0,
    Started = 1,
    CompleteOk = 2,
    CompleteErr = 3,
    Canceled = 4,
}

impl TaskState {
    pub(crate) fn from_u8(v: u8) -> Self {
        match v {
            1 => TaskState::Started,
            2 => TaskState::CompleteOk,
            3 => TaskState::CompleteErr,
            4 => TaskState::Canceled,
            _ => TaskState::Pending,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::CompleteOk | TaskState::CompleteErr | TaskState::Canceled
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CleanupState {
    Skipped = 0,
    Pending = 1,
    Running = 2,
    Success = 3,
    Failed = 4,
}

impl CleanupState {
    pub(crate) fn from_u8(v: u8) -> Self {
        match v {
            1 => CleanupState::Pending,
            2 => CleanupState::Running,
            3 => CleanupState::Success,
            4 => CleanupState::Failed,
            _ => CleanupState::Skipped,
        }
    }
}

const COPY_PHASE_SHARE: f32 = 0.75;
const RUNNING_CAP: f32 = 99.9;

#[derive(Debug, Clone)]
pub struct TaskSnapshot {
    pub state: TaskState,
    pub cleanup: CleanupState,
    pub total_bytes: u64,
    pub copied_bytes: u64,
    pub files_total: u32,
    pub files_ok: u32,
    pub files_failed: u32,
    /// entries not copied because of the conflict strategy
    pub files_skipped: u32,
    pub cleanup_done_files: u32,
}

impl TaskSnapshot {
    pub fn percent(&self) -> f32 {
        let copy = if self.total_bytes > 0 {
            self.copied_bytes as f32 / self.total_bytes as f32
        } else if self.files_total > 0 {
            (self.files_ok + self.files_failed) as f32 / self.files_total as f32
        } else {
            0.0
        };
        let p = if self.cleanup == CleanupState::Running {
            let denom = self.files_total.max(1) as f32;
            (copy * COPY_PHASE_SHARE
                + (self.cleanup_done_files as f32 / denom) * (1.0 - COPY_PHASE_SHARE))
                * 100.0
        } else {
            copy * 100.0
        };
        if self.state == TaskState::CompleteOk {
            return 100.0;
        }
        p.min(RUNNING_CAP)
    }
}

#[derive(Debug)]
pub(crate) struct Progress {
    state: AtomicU8,
    cleanup: AtomicU8,
    total_bytes: AtomicU64,
    copied_bytes: AtomicU64,
    files_total: AtomicU32,
    files_ok: AtomicU32,
    files_failed: AtomicU32,
    files_skipped: AtomicU32,
    cleanup_done_files: AtomicU32,
    move_cleanup: bool,
}

impl Progress {
    pub(crate) fn new(move_cleanup: bool) -> Self {
        Self {
            state: AtomicU8::new(TaskState::Pending as u8),
            cleanup: AtomicU8::new(if move_cleanup {
                CleanupState::Pending as u8
            } else {
                CleanupState::Skipped as u8
            }),
            total_bytes: AtomicU64::new(0),
            copied_bytes: AtomicU64::new(0),
            files_total: AtomicU32::new(0),
            files_ok: AtomicU32::new(0),
            files_failed: AtomicU32::new(0),
            files_skipped: AtomicU32::new(0),
            cleanup_done_files: AtomicU32::new(0),
            move_cleanup,
        }
    }

    pub(crate) fn state(&self) -> TaskState {
        TaskState::from_u8(self.state.load(Ordering::Acquire))
    }

    pub(crate) fn cas_state(
        &self,
        from: TaskState,
        to: TaskState,
    ) -> Result<(), TaskState> {
        match self
            .state
            .compare_exchange(from as u8, to as u8, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            Err(cur) => Err(TaskState::from_u8(cur)),
        }
    }

    pub(crate) fn register_file(
        &self,
        len: u64,
    ) {
        self.total_bytes.fetch_add(len, Ordering::Relaxed);
        self.files_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Registers `n` entries without byte accounting (extraction).
    pub(crate) fn register_entries(
        &self,
        n: u32,
    ) {
        self.files_total.fetch_add(n, Ordering::Relaxed);
    }

    /// Seeds the byte denominator directly (extraction formats that know
    /// the total up front); pairs with [`Self::add_copied`]. Idempotent:
    /// repeated seeds with the same value are no-ops.
    pub(crate) fn register_bytes(
        &self,
        n: u64,
    ) {
        self.total_bytes.fetch_max(n, Ordering::Relaxed);
    }

    pub(crate) fn add_copied(
        &self,
        n: u64,
    ) {
        self.copied_bytes.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn file_ok(&self) {
        self.files_ok.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn file_failed(&self) {
        self.files_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn skip_file(&self) {
        self.files_skipped.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn cleanup_started(&self) {
        if self.move_cleanup {
            let _ = self.cleanup.compare_exchange(
                CleanupState::Pending as u8,
                CleanupState::Running as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    pub(crate) fn cleanup_skip(&self) {
        self.cleanup
            .store(CleanupState::Skipped as u8, Ordering::Release);
    }

    pub(crate) fn cleanup_force_success(&self) {
        let cur = CleanupState::from_u8(self.cleanup.load(Ordering::Acquire));
        if matches!(cur, CleanupState::Pending | CleanupState::Running) {
            self.cleanup
                .store(CleanupState::Success as u8, Ordering::Release);
        }
    }

    pub(crate) fn cleanup_done(
        &self,
        n: u32,
    ) {
        self.cleanup_done_files.fetch_add(n, Ordering::Relaxed);
    }

    pub(crate) fn cleanup_failed(&self) {
        self.cleanup
            .store(CleanupState::Failed as u8, Ordering::Release);
    }

    pub(crate) fn snapshot(&self) -> TaskSnapshot {
        TaskSnapshot {
            state: self.state(),
            cleanup: CleanupState::from_u8(self.cleanup.load(Ordering::Acquire)),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            copied_bytes: self.copied_bytes.load(Ordering::Relaxed),
            files_total: self.files_total.load(Ordering::Relaxed),
            files_ok: self.files_ok.load(Ordering::Relaxed),
            files_failed: self.files_failed.load(Ordering::Relaxed),
            files_skipped: self.files_skipped.load(Ordering::Relaxed),
            cleanup_done_files: self.cleanup_done_files.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_bytes_weighted_and_capped() {
        let mut s = TaskSnapshot {
            state: TaskState::Started,
            cleanup: CleanupState::Skipped,
            total_bytes: 200,
            copied_bytes: 100,
            files_total: 2,
            files_ok: 1,
            files_failed: 0,
            files_skipped: 0,
            cleanup_done_files: 0,
        };
        assert!((s.percent() - 50.0).abs() < 1e-6);

        s.copied_bytes = 200;
        assert!((s.percent() - 99.9).abs() < 1e-6);

        s.state = TaskState::CompleteOk;
        assert_eq!(s.percent(), 100.0);
    }

    #[test]
    fn percent_move_cleanup_phase_blended() {
        let s = TaskSnapshot {
            state: TaskState::Started,
            cleanup: CleanupState::Running,
            total_bytes: 100,
            copied_bytes: 100,
            files_total: 4,
            files_ok: 4,
            files_failed: 0,
            files_skipped: 0,
            cleanup_done_files: 2,
        };
        let expected = 75.0 + 2.0 / 4.0 * 25.0;
        assert!((s.percent() - expected).abs() < 1e-5);
    }

    #[test]
    fn percent_zero_totals() {
        let s = TaskSnapshot {
            state: TaskState::Started,
            cleanup: CleanupState::Skipped,
            total_bytes: 0,
            copied_bytes: 0,
            files_total: 0,
            files_ok: 0,
            files_failed: 0,
            files_skipped: 0,
            cleanup_done_files: 0,
        };
        assert_eq!(s.percent(), 0.0);
    }
}
