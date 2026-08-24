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
    allow(dead_code, unused_variables)
)]

//! Scheduler-facing extraction runner.
//!
//! Bridges [`crate::work::ExtractJob`] to the format modules: detects the
//! archive, then runs the matching extract loop with entry-count progress
//! and cancellation. Per-entry failures are recorded and skipped; only
//! structural errors (undetectable format, unreadable archive, cancel)
//! abort with an error.

use std::sync::Arc;

use crate::error::WorkError;
use crate::extract::ctx::ExtractCtx;
use crate::scheduler::JobCtx;
use crate::work::ExtractJob;

use super::{Format, detect};

pub(crate) fn run(
    job: &Arc<JobCtx>,
    work: &ExtractJob,
) -> Result<(), WorkError> {
    let Some(format) = detect(&work.source) else {
        return Err(WorkError::Io(std::io::Error::other(format!(
            "no enabled backend recognizes {}",
            work.source.display()
        ))));
    };
    log::info!(
        "extracting {} ({}) -> {}",
        work.source.display(),
        format.id(),
        work.dest.display()
    );
    let ctx = ExtractCtx::of_job(job);
    let res = extract(work, format, &ctx);
    if ctx.cancelled() {
        return Err(WorkError::Canceled);
    }
    res.map_err(WorkError::Io)
}

fn extract(
    work: &ExtractJob,
    format: Format,
    ctx: &ExtractCtx<'_>,
) -> std::io::Result<()> {
    match format {
        #[cfg(feature = "zip")]
        Format::Zip => super::zip::extract(&work.source, &work.dest, ctx),
        #[cfg(feature = "tar")]
        Format::Tar => {
            super::tarball::extract(super::tarball::source(&work.source)?, &work.dest, ctx)
        }
        #[cfg(feature = "ar")]
        Format::Ar => super::ar::extract(&work.source, &work.dest, ctx),
        #[cfg(feature = "rar")]
        Format::Rar => super::rar::extract(&work.source, &work.dest, ctx),
        #[cfg(feature = "sevenz")]
        Format::SevenZ => super::sevenz::extract(&work.source, &work.dest, ctx),
        #[cfg(any(feature = "bz2", feature = "gz", feature = "xz", feature = "zst"))]
        Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => {
            super::stream::extract(&work.source, &work.dest, format, ctx)
        }
        #[allow(unreachable_patterns)]
        _ => Err(std::io::Error::other(format!(
            "no backend compiled for {}",
            format.id()
        ))),
    }
}
