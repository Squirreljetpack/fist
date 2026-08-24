//! Directory-only preview trees for archive listings.
//!
//! Before an archive extracts in the background, the caller can
//! materialize its directory structure synchronously so navigation shows
//! the full layout immediately. Only directories are created — regular
//! files, links, and anything with an unsafe path are never touched.

use std::fs;
use std::path::Path;

use super::ArchiveEntry;
use super::safety::is_safe;

/// Creates under `dest` every directory implied by `entries`: explicit
/// directory entries plus the parent chains of file entries. Unsafe entry
/// paths are skipped (warned), matching extraction behavior; I/O errors on
/// individual directories are tolerated so one failure cannot block the
/// rest of the preview.
pub fn skeleton(
    dest: &Path,
    entries: &[ArchiveEntry],
) {
    for entry in entries {
        let path = entry.path.as_path();
        if !is_safe(path) || path.as_os_str().is_empty() {
            log::warn!("skeleton: skipping unsafe entry {:?}", entry.path);
            continue;
        }
        if entry.is_dir {
            if let Err(e) = fs::create_dir_all(dest.join(path)) {
                log::warn!("skeleton: could not create {:?}: {e}", path);
            }
        } else if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            // parents of every file form the directory tree, which also
            // covers listings that drop explicit dir entries (zip/tar)
            if let Err(e) = fs::create_dir_all(dest.join(parent)) {
                log::warn!("skeleton: could not create {:?}: {e}", parent);
            }
        }
    }
}
