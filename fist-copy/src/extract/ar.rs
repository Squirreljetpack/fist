//! `ar` archives via the `ar` crate.
//!
//! A flat, uncompressed member format: every entry is a file, names may
//! carry a trailing `/` (GNU convention) and non-UTF8 bytes.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;

use super::ArchiveEntry;
use super::ctx::ExtractCtx;
use super::safety::is_safe;

/// Decodes a member name: UTF-8 lossy, trailing `/` stripped.
fn member_name(raw: &[u8]) -> String {
    let name = String::from_utf8_lossy(raw);
    name.strip_suffix('/').unwrap_or(&name).to_owned()
}

pub(crate) fn list(source: &Path) -> io::Result<Vec<ArchiveEntry>> {
    let mut archive = ar::Archive::new(File::open(source)?);
    let mut out = Vec::new();
    while let Some(entry) = archive.next_entry() {
        let entry = entry?;
        out.push(ArchiveEntry {
            path: member_name(entry.header().identifier()).into(),
            is_dir: false,
        });
    }
    Ok(out)
}

/// Extracts every member into `dest` as a plain file. The flat
/// uncompressed layout makes source-byte consumption exact progress.
pub(crate) fn extract(
    source: &Path,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
) -> io::Result<()> {
    let (file, sb) = super::codec::track_source(source)?;
    sb.report(ctx);
    let mut archive = ar::Archive::new(file);
    while let Some(entry) = archive.next_entry() {
        if ctx.cancelled() {
            return Err(super::ctx::cancelled());
        }
        ctx.register_entries(1);
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!("ar: unreadable entry: {e}");
                ctx.entry_failed();
                continue;
            }
        };
        let rel = member_name(entry.header().identifier());
        if rel.is_empty() || !is_safe(Path::new(&rel)) {
            log::warn!("ar: skipping unsafe entry {rel:?}");
            ctx.entry_skipped();
            continue;
        }
        let full = dest.join(&rel);
        let res = write_member(entry, &full);
        match res {
            Ok(()) => ctx.entry_ok(),
            Err(e) => {
                log::warn!("ar: failed to extract {rel:?}: {e}");
                ctx.entry_failed();
            }
        }
        sb.report(ctx);
    }
    sb.finish(ctx);
    Ok(())
}

fn write_member<R: Read>(
    entry: ar::Entry<'_, R>,
    full: &Path,
) -> io::Result<()> {
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    // header size is authoritative; the entry reads exactly that many bytes
    let len = entry.header().size();
    let mut out = File::create(full)?;
    io::copy(&mut entry.take(len), &mut out)?;
    drop(out);
    Ok(())
}
