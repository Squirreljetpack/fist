#![cfg_attr(
    not(any(
        feature = "zip",
        feature = "tar",
        feature = "gz",
        feature = "bz2",
        feature = "xz",
        feature = "zst",
        feature = "ar",
        feature = "rar",
        feature = "sevenz"
    )),
    allow(dead_code)
)]

//! Extraction execution context: entry-count progress and cancellation.
//!
//! Format modules receive [`ExtractCtx`] instead of any scheduler type, so
//! they stay independent of the worker machinery and testable in isolation.
//! Accounting is entry-counted: every listed entry registers, every
//! extracted entry resolves ok / failed / skipped.

use std::sync::Arc;

use crate::progress::Progress;
use crate::scheduler::JobCtx;
use crate::token::CancelToken;

#[derive(Clone, Copy)]
pub(crate) struct ExtractCtx<'a> {
    token: &'a CancelToken,
    prog: &'a Progress,
}

impl<'a> ExtractCtx<'a> {
    pub(crate) fn new(
        token: &'a CancelToken,
        prog: &'a Progress,
    ) -> Self {
        Self { token, prog }
    }

    /// Builds from a running scheduler job.
    pub(crate) fn of_job(job: &'a Arc<JobCtx>) -> Self {
        Self::new(&job.token, &job.prog)
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Registers `n` more known entries up front (listing already ran).
    pub(crate) fn register_entries(
        &self,
        n: u32,
    ) {
        self.prog.register_entries(n);
    }

    /// Seeds the byte denominator for formats that know it up front.
    pub(crate) fn register_bytes(
        &self,
        total: u64,
    ) {
        self.prog.register_bytes(total);
    }

    /// Accounts payload bytes as they are produced; once the denominator is
    /// seeded, percent derives from bytes instead of entry counts.
    pub(crate) fn add_copied(
        &self,
        n: u64,
    ) {
        self.prog.add_copied(n);
    }

    /// Marks one registered entry resolved successfully.
    pub(crate) fn entry_ok(&self) {
        self.prog.file_ok();
    }

    /// Marks one registered entry failed.
    pub(crate) fn entry_failed(&self) {
        self.prog.file_failed();
    }

    /// Marks one registered entry skipped (unsafe path, conflict policy).
    pub(crate) fn entry_skipped(&self) {
        self.prog.skip_file();
    }
}

/// The canonical mid-extraction cancellation error.
pub(crate) fn cancelled() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Interrupted, "extraction cancelled")
}
