//! Zip archives via the `zip` crate.
//!
//! Entries are extracted independently (random access via the central
//! directory), unix permissions are applied from the external attributes,
//! and entry paths come pre-validated by `enclosed_name` (zip-slip guard).

use std::fs::{self, File};
use std::io::{self, Read, Write};
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
    let archive = open(source)?;
    Ok(archive
        .file_names()
        .map(|name| ArchiveEntry {
            is_dir: name.ends_with('/') || name.ends_with('\\'),
            path: name.trim_end_matches(['/', '\\']).into(),
        })
        .collect())
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
    let total = match archive.decompressed_size() {
        Some(t) => t as u64,
        None => {
            let mut sum = 0u64;
            for i in 0..archive.len() {
                if let Ok(entry) = archive.by_index(i) {
                    sum += entry.size();
                }
            }
            sum
        }
    };
    ctx.register_bytes(total);
    ctx.register_entries(archive.len() as u32);

    for i in 0..archive.len() {
        if ctx.cancelled() {
            return Err(super::ctx::cancelled());
        }
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
        } else if entry.is_symlink() {
            match write_symlink(&mut entry, &rel, dest, &full) {
                Ok(n) => {
                    ctx.add_copied(n);
                    Ok(n)
                }
                Err(e) => Err(e),
            }
        } else {
            write_entry(&mut entry, &full, ctx)
        };
        match res {
            Ok(_) => {
                ctx.entry_ok();
            }
            Err(e) => {
                if ctx.cancelled() {
                    return Err(super::ctx::cancelled());
                }
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

/// Creates the symlink the entry describes. The payload *is* the target
/// path; it is validated against traversal exactly like archive paths.
/// Non-unix targets fall back to a plain file containing the target.
fn write_symlink<R>(
    entry: &mut zip::read::ZipFile<'_, R>,
    rel: &Path,
    dest: &Path,
    full: &Path,
) -> io::Result<u64>
where
    R: std::io::Read + std::io::Seek,
{
    let mut target = String::new();
    entry.read_to_string(&mut target)?;

    #[cfg(unix)]
    {
        let link_dir = rel.parent().unwrap_or(Path::new(""));
        if !super::safety::link_target_safe(dest, link_dir, Path::new(&target)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("link target escapes destination: {target:?}"),
            ));
        }
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::remove_file(full);
        std::os::unix::fs::symlink(&target, full)?;
        Ok(target.len() as u64)
    }
    #[cfg(not(unix))]
    {
        let _ = (rel, dest);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, target.as_bytes())?;
        Ok(target.len() as u64)
    }
}

fn write_entry<R>(
    entry: &mut zip::read::ZipFile<'_, R>,
    full: &Path,
    ctx: &ExtractCtx<'_>,
) -> io::Result<u64>
where
    R: std::io::Read + std::io::Seek,
{
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = File::create(full)?;
    let mut buf = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        if ctx.cancelled() {
            return Err(super::ctx::cancelled());
        }
        let n = entry.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        written += n as u64;
        ctx.add_copied(n as u64);
    }
    out.flush()?;
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
    Ok(written)
}
