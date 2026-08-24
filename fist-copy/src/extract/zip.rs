//! Zip archives via the `zip` crate.
//!
//! Entries are extracted independently (random access via the central
//! directory), unix permissions are applied from the external attributes,
//! and entry paths come pre-validated by `enclosed_name` (zip-slip guard).

use std::fs::{self, File};
use std::io;
use std::path::Path;

use super::ArchiveEntry;
use super::ctx::ExtractCtx;

fn open(source: &Path) -> io::Result<zip::ZipArchive<File>> {
    let file = File::open(source)?;
    zip::ZipArchive::new(file).map_err(|e| io::Error::other(e.to_string()))
}

/// Lists every entry; names that fail the traversal check surface as
/// entries with empty paths so callers can count them.
pub(crate) fn list(source: &Path) -> io::Result<Vec<ArchiveEntry>> {
    let mut archive = open(source)?;
    let mut out = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        out.push(ArchiveEntry {
            path: enclosed(&entry).unwrap_or_default(),
            is_dir: entry.is_dir(),
        });
    }
    Ok(out)
}

/// Extracts every entry into `dest`. Per-entry failures are recorded and
/// skipped; only cancellation or a structural read error aborts the job.
/// The central directory's uncompressed sizes make byte progress exact.
pub(crate) fn extract(
    source: &Path,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
) -> io::Result<()> {
    let mut archive = open(source)?;
    // the central directory is already parsed; summing sizes is cheap
    let mut total = 0u64;
    for i in 0..archive.len() {
        if let Ok(entry) = archive.by_index(i) {
            total += entry.size();
        }
    }
    ctx.register_bytes(total);

    for i in 0..archive.len() {
        if ctx.cancelled() {
            return Err(super::ctx::cancelled());
        }
        ctx.register_entries(1);
        let mut entry = match archive.by_index(i) {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!("zip: unreadable entry {i}: {e}");
                ctx.entry_failed();
                continue;
            }
        };
        let Some(rel) = enclosed(&entry) else {
            log::warn!("zip: skipping unsafe entry {:?}", entry.name());
            ctx.entry_skipped();
            continue;
        };
        let full = dest.join(&rel);
        let res = if entry.is_dir() {
            fs::create_dir_all(&full).map(|_| 0u64)
        } else {
            write_entry(&mut entry, &full)
        };
        match res {
            Ok(n) => {
                ctx.entry_ok();
                ctx.add_copied(n);
            }
            Err(e) => {
                log::warn!("zip: failed to extract {rel:?}: {e}");
                ctx.entry_failed();
            }
        }
    }
    Ok(())
}

/// The entry's validated relative path (`None` when it escapes `dest`).
fn enclosed<R>(entry: &zip::read::ZipFile<'_, R>) -> Option<std::path::PathBuf>
where
    R: std::io::Read + std::io::Seek,
{
    entry.enclosed_name().map(|p| p.to_path_buf())
}

fn write_entry<R>(
    entry: &mut zip::read::ZipFile<'_, R>,
    full: &Path,
) -> io::Result<u64>
where
    R: std::io::Read + std::io::Seek,
{
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = File::create(full)?;
    let n = io::copy(entry, &mut out)?;
    drop(out);
    if let Some(mode) = entry.unix_mode() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(full, fs::Permissions::from_mode(mode));
        }
        #[cfg(not(unix))]
        {
            let _ = mode;
        }
    }
    Ok(n)
}
