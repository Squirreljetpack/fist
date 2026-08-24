//! Archive extraction engine.
//!
//! Formats are detected by extension with a magic-byte fallback
//! ([`detect`]), listed entry-wise ([`list`]), and extracted by the copy
//! worker pool as [`crate::work::WorkItem::Extract`] jobs, reporting
//! entry-count progress through [`crate::progress::Progress`] and honoring
//! [`crate::token::CancelToken`] between entries.
//!
//! Each format family lives in its own submodule behind a cargo feature of
//! the same name; a format whose feature is disabled is never detected.
//! Compressed streams (gz/bz2/xz/zst) peek the decoded head for tar magic,
//! so compound archives (tar.gz & co) and bare compressed files are told
//! apart by content rather than file name.

pub(crate) mod ctx;
pub(crate) mod runner;
mod safety;

#[cfg(feature = "ar")]
mod ar;
mod codec;
mod detect;
#[cfg(feature = "rar")]
mod rar;
#[cfg(feature = "sevenz")]
mod sevenz;
#[cfg(any(feature = "bz2", feature = "gz", feature = "xz", feature = "zst"))]
mod stream;
#[cfg(feature = "tar")]
mod tarball;
#[cfg(feature = "zip")]
mod zip;

pub use detect::{Format, detect};

use std::io;
use std::path::Path;

/// One entry in an archive listing.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Entry path relative to the archive root.
    pub path: std::path::PathBuf,
    pub is_dir: bool,
}

/// Listing failed.
#[derive(Debug, thiserror::Error)]
#[error("failed to list {archive}: {source}")]
pub struct ListError {
    /// The archive that could not be listed.
    pub archive: std::path::PathBuf,
    #[source]
    pub source: io::Error,
}

/// Lists every entry of `source`, cheaply where the format allows.
///
/// The format must be enabled ([`detect`] returns `Some`); detection is not
/// repeated here.
pub fn list(
    source: &Path,
    format: Format,
) -> Result<Vec<ArchiveEntry>, ListError> {
    let inner = || -> io::Result<Vec<ArchiveEntry>> {
        match format {
            #[cfg(feature = "zip")]
            Format::Zip => zip::list(source),
            #[cfg(feature = "tar")]
            Format::Tar => tarball::list(tarball::source(source)?),
            #[cfg(feature = "ar")]
            Format::Ar => ar::list(source),
            #[cfg(feature = "rar")]
            Format::Rar => rar::list(source),
            #[cfg(feature = "sevenz")]
            Format::SevenZ => sevenz::list(source),
            #[cfg(any(feature = "bz2", feature = "gz", feature = "xz", feature = "zst"))]
            Format::Gz | Format::Bz2 | Format::Xz | Format::Zst => stream::list(source, format),
        }
    };
    inner().map_err(|e| ListError {
        archive: source.to_path_buf(),
        source: e,
    })
}
