//! Archive detection for the `decompress` backend.
//!
//! Detection is content-based: the file magic (first 8 KB) is sniffed via
//! `infer`, which covers zip, the tar family, ar, gz, bz2, xz, zstd, and rar
//! regardless of the file name.

use std::path::Path;

/// Whether the `decompress` backend recognizes `path` by its content.
pub fn is_decompress_archive(path: &Path) -> bool {
    decompress::can_decompress_content(path).unwrap_or(false)
}
