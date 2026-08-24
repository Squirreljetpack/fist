//! 7z archives via `sevenz-rust2`.
//!
//! Listing reads the header only. Extraction v1 is whole-archive
//! ([`sevenz_rust2::decompress_file`]) — the format has no cheap
//! entry-wise random access through this crate — so progress resolves all
//! entries after the single extraction call and cancellation applies
//! before it starts, not mid-archive.

use std::fs;
use std::io;
use std::path::Path;

use super::ctx::ExtractCtx;
use super::{ArchiveEntry, safety};

fn open(source: &Path) -> io::Result<sevenz_rust2::Archive> {
    sevenz_rust2::Archive::open(source).map_err(|e| io::Error::other(e.to_string()))
}

pub(crate) fn list(source: &Path) -> io::Result<Vec<ArchiveEntry>> {
    let archive = open(source)?;
    Ok(archive
        .files
        .iter()
        .map(|entry| ArchiveEntry {
            path: entry.name().into(),
            is_dir: entry.is_directory(),
        })
        .collect())
}

/// Extracts the whole archive into `dest` as one operation.
pub(crate) fn extract(
    source: &Path,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
) -> io::Result<()> {
    let total = {
        let archive = open(source)?;
        archive.files.len() as u32
    };
    if ctx.cancelled() {
        return Err(super::ctx::cancelled());
    }
    // decompress_file has no per-entry hook to intercept writes, so any
    // unsafe entry refuses the whole archive rather than risking an escape
    ctx.register_entries(total);
    let unsafe_count = count_unsafe(source)?;
    if unsafe_count > 0 {
        for _ in 0..total {
            ctx.entry_failed();
        }
        return Err(io::Error::other(format!(
            "refusing 7z with {unsafe_count} unsafe path(s)"
        )));
    }
    fs::create_dir_all(dest)?;
    match sevenz_rust2::decompress_file(source, dest) {
        Ok(()) => {
            for _ in 0..total {
                ctx.entry_ok();
            }
            Ok(())
        }
        Err(e) => {
            // everything unresolved counts failed so the task ends in an
            // error state rather than a phantom partial success
            for _ in 0..total {
                ctx.entry_failed();
            }
            Err(io::Error::other(e.to_string()))
        }
    }
}

fn count_unsafe(source: &Path) -> io::Result<u32> {
    let archive = open(source)?;
    Ok(archive
        .files
        .iter()
        .filter(|e| !safety::is_safe(Path::new(e.name())))
        .count() as u32)
}
