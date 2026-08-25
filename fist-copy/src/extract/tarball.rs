//! The tar family: plain tars and every tar.<compression> compound.
//!
//! All I/O goes through a byte source ([`ByteSource`]) so the same entry
//! iteration serves plain files, decoded streams, and stream prefixes
//! already peeked by [`super::codec::peek_tar`].

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use tar::{Archive, Entry, EntryType};

use super::ArchiveEntry;
use super::codec::{self, SourceBytes};
use super::ctx::ExtractCtx;
use super::safety::{is_safe, link_target_safe};

/// A rewind-free byte source: either a real file or a decoded stream that
/// may start with an already-consumed peek block.
pub(crate) enum ByteSource {
    File(Box<dyn Read + Send>),
    Stream {
        /// Bytes consumed during content sniffing, replayed first.
        prefix: Box<dyn Read + Send>,
        inner: codec::BoxDecoder,
    },
}

impl Read for ByteSource {
    fn read(
        &mut self,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        match self {
            ByteSource::File(f) => f.read(buf),
            ByteSource::Stream { prefix, inner } => match prefix.read(buf)? {
                0 => inner.read(buf),
                n => Ok(n),
            },
        }
    }
}

/// Opens `source` as a plain (uncompressed) tar.
pub(crate) fn source(source: &Path) -> io::Result<ByteSource> {
    Ok(ByteSource::File(Box::new(fs::File::open(source)?)))
}

/// Builds a plain-tar source over an existing reader (a tracked file for
/// source-byte accounting).
pub(crate) fn source_reader(reader: impl Read + Send + 'static) -> ByteSource {
    ByteSource::File(Box::new(reader))
}

/// Wraps a decoded stream whose first block was consumed by sniffing.
pub(crate) fn source_from_decoded(
    prefix_block: [u8; codec::TAR_BLOCK],
    decoded: codec::BoxDecoder,
) -> ByteSource {
    ByteSource::Stream {
        prefix: Box::new(io::Cursor::new(prefix_block)),
        inner: decoded,
    }
}

/// Lists every safe entry of the archive.
pub(crate) fn list(reader: ByteSource) -> io::Result<Vec<ArchiveEntry>> {
    let mut out = Vec::new();
    for_meta(reader, |path, header| {
        out.push(ArchiveEntry {
            path,
            is_dir: header.entry_type().is_dir(),
        });
        Ok(())
    })?;
    Ok(out)
}

/// Extracts the archive into `dest`, one progress unit per entry. When
/// `bytes` is given, source consumption is folded into byte progress after
/// every entry.
pub(crate) fn extract(
    reader: ByteSource,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
    bytes: Option<&SourceBytes>,
) -> io::Result<()> {
    for_each(reader, dest, ctx, bytes, |entry, full, _| {
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&full)?;
            return Ok(());
        }
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(&full)?;
        Ok(())
    })
}

// -------------------------- iteration core --------------------------

fn for_meta(
    reader: ByteSource,
    mut f: impl FnMut(PathBuf, &tar::Header) -> io::Result<()>,
) -> io::Result<()> {
    let mut archive = Archive::new(reader);
    for entry in archive.entries()? {
        let entry = entry?;
        let path = normalize(&entry)?;
        f(path, entry.header())?;
    }
    Ok(())
}

fn for_each(
    reader: ByteSource,
    dest: &Path,
    ctx: &ExtractCtx<'_>,
    bytes: Option<&SourceBytes>,
    mut write: impl FnMut(&mut Entry<'_, ByteSource>, PathBuf, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let mut archive = Archive::new(reader);
    for entry in archive.entries()? {
        if ctx.cancelled() {
            return Err(super::ctx::cancelled());
        }
        // registered before anything is inspected: failed and skipped
        // entries are part of the total, like the other formats
        ctx.register_entries(1);
        let mut entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!("tar: unreadable entry: {e}");
                ctx.entry_failed();
                continue;
            }
        };
        let rel = match normalize(&entry) {
            Ok(rel) => rel,
            Err(e) => {
                log::warn!("tar: bad entry path: {e}");
                ctx.entry_skipped();
                continue;
            }
        };
        let ty = entry.header().entry_type();
        if !is_safe(&rel)
            || (matches!(ty, EntryType::Symlink | EntryType::Link)
                && !link_target_is_safe(dest, &rel, &entry))
        {
            log::warn!("tar: skipping unsafe entry {rel:?}");
            ctx.entry_skipped();
            continue;
        }
        let full = dest.join(&rel);
        match write(&mut entry, full, dest) {
            Ok(()) => ctx.entry_ok(),
            Err(e) => {
                log::warn!("tar: failed to extract {rel:?}: {e}");
                ctx.entry_failed();
            }
        }
        if let Some(bytes) = bytes {
            bytes.report(ctx);
        }
    }
    Ok(())
}

/// Resolves the entry's path with Windows separators normalized away.
fn normalize<R: Read>(entry: &Entry<'_, R>) -> io::Result<PathBuf> {
    let raw = entry.path()?.to_path_buf();
    // tar on some platforms keeps `\`; treat it as `/` like libarchive does
    let s = raw.to_string_lossy().replace('\\', "/");
    Ok(PathBuf::from(s))
}

fn link_target_is_safe<R: Read>(
    dest: &Path,
    rel: &Path,
    entry: &Entry<'_, R>,
) -> bool {
    let target = entry.link_name().ok().flatten();
    match target {
        // the OS resolves the target relative to the link's directory
        Some(t) => link_target_safe(dest, rel.parent().unwrap_or(Path::new("")), &t),
        None => false,
    }
}
