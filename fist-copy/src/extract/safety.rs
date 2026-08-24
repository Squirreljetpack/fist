#![cfg_attr(
    not(any(
        feature = "tar",
        feature = "gz",
        feature = "bz2",
        feature = "xz",
        feature = "zst",
        feature = "ar",
        feature = "rar",
        feature = "sevenz"
    )),
    allow(dead_code)
)]

//! Path safety for archive entries.
//!
//! Archive metadata is never trusted: entry paths must be relative with no
//! `..` components, and link targets must resolve inside the extraction
//! destination.

use std::path::{Component, Path, PathBuf};

/// True when `path` is relative and contains no `..` components.
pub(crate) fn is_safe(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// True when a link at `link_dir` pointing at `target` stays inside
/// `root`. Absolute targets are rejected; relative targets are joined onto
/// the link's directory (without requiring it to exist).
pub(crate) fn link_target_safe(
    root: &Path,
    link_dir: &Path,
    target: &Path,
) -> bool {
    if target.is_absolute() {
        return false;
    }
    let mut joined: PathBuf = if link_dir.is_absolute() {
        link_dir.to_path_buf()
    } else {
        root.join(link_dir)
    };
    for c in target.components() {
        match c {
            Component::Normal(_) => joined.push(c),
            Component::ParentDir => {
                if !joined.pop() {
                    return false;
                }
            }
            Component::CurDir => {}
            _ => return false,
        }
        if !joined.starts_with(root) {
            return false;
        }
    }
    true
}
