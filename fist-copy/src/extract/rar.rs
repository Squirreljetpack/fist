//! RAR archives via the `unrar` crate (C++ RAR SDK bindings).
//!
//! Extraction walks the archive front to back through the crate's cursor
//! API: one `read_header` / `extract_to` step per entry, which gives
//! entry-wise progress and a cancellation check between entries. Unsafe
//! entry paths are skipped. The cursor is consuming, so the first
//! extraction error aborts the remaining archive (the task still ends in
//! an error state).

use std::io;
use std::path::{Path, PathBuf};

use unrar::Archive as RarFile;

use super::ctx::ExtractCtx;
use super::{ArchiveEntry, safety};

pub(crate) fn list(source: &Path) -> io::Result<Vec<ArchiveEntry>> {
    let archive = RarFile::new(source)
        .open_for_listing()
        .map_err(io::Error::other)?;
    let mut out = Vec::new();
    for entry in archive {
        let entry = entry.map_err(io::Error::other)?;
        out.push(ArchiveEntry {
            path: entry.filename.to_path_buf(),
            is_dir: entry.is_directory(),
        });
    }
    Ok(out)
}

pub(crate) fn extract(
    source: &Path,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
) -> io::Result<()> {
    // the cursor methods consume the archive; `rest` threads it through
    let mut rest = Some(
        RarFile::new(source)
            .open_for_processing()
            .map_err(io::Error::other)?,
    );

    while let Some(archive) = rest.take() {
        if ctx.cancelled() {
            return Err(super::ctx::cancelled());
        }
        let Some(file) = archive.read_header().map_err(io::Error::other)? else {
            break;
        };
        ctx.register_entries(1);

        let rel: PathBuf = file.entry().filename.clone();
        if rel.as_os_str().is_empty() || !safety::is_safe(&rel) {
            log::warn!("rar: skipping unsafe entry {:?}", rel);
            ctx.entry_skipped();
            rest = file.skip().map(Some).map_err(io::Error::other)?;
            continue;
        }

        match file.extract_with_base(dest) {
            Ok(next) => {
                ctx.entry_ok();
                rest = Some(next);
            }
            Err(e) => {
                log::warn!(
                    "rar: failed to extract {:?}: {e}; aborting remaining entries",
                    rel
                );
                ctx.entry_failed();
                break;
            }
        }
    }
    Ok(())
}
