//! Metadata sort helpers for the non-interactive (`--list`) paths.
//!
//! The interactive pane sorts through nucleo ([`crate::run::state::sort`]);
//! these helpers are the `--list` equivalents. Size is computed through the
//! shared [`fist_size::DirSizeCache`] (see [`sort_by_size`]) so every size
//! read shares one source with the interactive pane.
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

use crate::run::state::sort;

/// Sort by mtime, newest first. Errors sort first.
pub fn sort_by_mtime(paths: &mut [PathBuf]) {
    paths.sort_by(|a, b| {
        let ma = std::fs::metadata(a).and_then(|m| m.modified());
        let mb = std::fs::metadata(b).and_then(|m| m.modified());
        match (ma, mb) {
            (Ok(ma), Ok(mb)) => mb.cmp(&ma).then_with(|| a.cmp(b)), // descending
            (Err(_), Ok(_)) => Ordering::Less,
            (Ok(_), Err(_)) => Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        }
    });
}

/// Sort by atime, newest first. Errors sort last.
pub fn sort_by_atime(paths: &mut [PathBuf]) {
    paths.sort_by(|a, b| {
        let aa = std::fs::metadata(a).and_then(|m| m.accessed());
        let ab = std::fs::metadata(b).and_then(|m| m.accessed());
        match (aa, ab) {
            (Ok(aa), Ok(ab)) => ab.cmp(&aa).then_with(|| a.cmp(b)), // descending
            (Ok(_), Err(_)) => Ordering::Less,
            (Err(_), Ok(_)) => Ordering::Greater,
            (Err(_), Err(_)) => a.cmp(b),
        }
    });
}

/// Sort by size, largest first, computed through the shared
/// [`crate::run::state::sort::dir_size`] cache.
///
/// `--list` runs in its own process, so the cache starts empty and there is
/// no cross-invocation staleness. Missing entries read as `0`, which sorts
/// last under the descending order. Blocking `wait()` is fine: `--list` is
/// fully synchronous.
pub fn sort_by_size(paths: &mut [PathBuf]) {
    let cache = sort::dir_size();
    for p in paths.iter() {
        cache.add(p);
    }
    cache.wait();
    paths.sort_by(|a, b| {
        let sa = sort::size_of(a).unwrap_or(0);
        let sb = sort::size_of(b).unwrap_or(0);
        sb.cmp(&sa).then_with(|| a.cmp(b)) // descending
    });
}

pub fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
