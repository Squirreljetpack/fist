//! Compressed streams: gz / bz2 / xz / zst.
//!
//! One decode pass peeks the first block for tar magic — compound archives
//! (tar.gz & co) delegate to [`super::tarball`], bare compressed files are
//! copied through the decoder under their de-suffixed name.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::ArchiveEntry;
use super::codec;
use super::ctx::ExtractCtx;
use super::detect::Format;
use super::tarball::{self, ByteSource};

enum Shape {
    /// The stream decodes into a tar archive.
    Tar(ByteSource),
    /// The stream decodes to a single bare file.
    Bare(Box<dyn Read + Send>),
}

/// Classifies the decoded stream, returning either a tar source ready for
/// iteration or a bare reader (with its peek block already consumed).
fn shape(
    source: impl Read + Send + 'static,
    format: Format,
) -> io::Result<Shape> {
    let mut decoded = codec::decode(source, format);
    let (is_tar, block) = codec::peek_tar(&mut decoded)?;
    if is_tar {
        Ok(Shape::Tar(tarball::source_from_decoded(block, decoded)))
    } else {
        Ok(Shape::Bare(Box::new(io::Cursor::new(block).chain(decoded))))
    }
}

pub(crate) fn list(
    source: &Path,
    format: Format,
) -> io::Result<Vec<ArchiveEntry>> {
    match shape(File::open(source)?, format)? {
        Shape::Tar(reader) => tarball::list(reader),
        Shape::Bare(_) => Ok(vec![ArchiveEntry {
            path: PathBuf::from(bare_stem(source)),
            is_dir: false,
        }]),
    }
}

pub(crate) fn extract(
    source: &Path,
    dest: &Path,
    format: Format,
    ctx: &ExtractCtx<'_>,
) -> io::Result<()> {
    // tracked at the raw layer: consumed compressed bytes vs file size
    let (file, sb) = codec::track_source(source)?;
    sb.report(ctx);
    match shape(file, format)? {
        Shape::Tar(reader) => {
            let res = tarball::extract(reader, dest, ctx, Some(&sb));
            if res.is_ok() {
                sb.finish(ctx);
            }
            res
        }
        Shape::Bare(mut decoded) => {
            if ctx.cancelled() {
                return Err(super::ctx::cancelled());
            }
            ctx.register_entries(1);
            let full = dest.join(bare_stem(source));
            fs::create_dir_all(dest)?;
            let mut out = File::create(&full)?;
            // chunked so cancellation lands between chunks and byte
            // progress advances during the copy
            let mut buf = vec![0u8; 64 * 1024];
            loop {
                if ctx.cancelled() {
                    return Err(super::ctx::cancelled());
                }
                let n = decoded.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                sb.report(ctx);
            }
            drop(out);
            ctx.entry_ok();
            sb.finish(ctx);
            Ok(())
        }
    }
}

/// Output file name for a bare stream: the file name minus its
/// compression suffix (`notes.txt.gz` -> `notes.txt`), falling back to
/// appending `.out` when nothing can be stripped.
fn bare_stem(source: &Path) -> String {
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "archive".into());
    for sfx in [
        ".gz", ".bz2", ".xz", ".zst", ".tgz", ".taz", ".tbz", ".tbz2", ".txz", ".tzst",
    ] {
        if name.len() > sfx.len()
            && let Some(stem) = name.strip_suffix(sfx)
        {
            return stem.to_owned();
        }
    }
    format!("{name}.out")
}
