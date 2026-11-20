// `zip` is deliberately absent: a zip-only build uses `link_target_safe` (on
// unix) and the helpers common to every format, but not `resolve_existing`,
// so it still needs the allowance. Adding `zip` here would break a zip-only
// build with a dead-code warning.
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

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// True when `path` is non-empty, relative, and contains no `..`
/// components.
pub(crate) fn is_safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Creates `root` when needed and returns its canonical path.
pub(crate) fn canonical_root(root: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    fs::canonicalize(root)
}

/// The `Normal` components of `rel`; [`is_safe`] rejects anything else.
fn components(rel: &Path) -> Vec<&std::ffi::OsStr> {
    rel.components()
        .filter_map(|c| match c {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect()
}

/// Walks `rel`'s parent components under `root`, resolving and validating
/// symlinks. Missing directories are created only when `create` is set; the
/// leaf is never touched.
fn walk_parents(
    root: &Path,
    parents: &[&std::ffi::OsStr],
    create: bool,
) -> io::Result<PathBuf> {
    let mut cur = root.to_path_buf();
    for name in parents.iter().copied() {
        let next = cur.join(name);
        cur = match fs::symlink_metadata(&next) {
            Ok(md) if md.file_type().is_symlink() => {
                let resolved = fs::canonicalize(&next)?;
                if !resolved.starts_with(root) {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        format!("extraction path escapes destination: {}", next.display()),
                    ));
                }
                if !resolved.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        format!("extraction parent is not a directory: {}", next.display()),
                    ));
                }
                resolved
            }
            Ok(md) if md.is_dir() => next,
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("extraction parent is not a directory: {}", next.display()),
                ));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if create {
                    fs::create_dir(&next)?;
                }
                next
            }
            Err(e) => return Err(e),
        };
    }
    Ok(cur)
}

/// Resolves `rel`'s parent under `root` without creating directories or
/// touching the leaf. Lets callers validate a link target before a rejected
/// entry can change the destination.
pub(crate) fn resolve_parent(
    root: &Path,
    rel: &Path,
) -> io::Result<PathBuf> {
    debug_assert!(is_safe(rel), "unsafe extraction path: {rel:?}");
    let comps = components(rel);
    let Some((_, parents)) = comps.split_last() else {
        return Ok(root.to_path_buf());
    };
    walk_parents(root, parents, false)
}

/// Resolves `rel` under the canonical `root`, creating parent directories.
///
/// Existing symlink components are resolved and must stay inside `root`.
/// A symlink at the leaf is removed so a following create/open cannot
/// follow it. `rel` must satisfy [`is_safe`].
pub(crate) fn prepare_path(
    root: &Path,
    rel: &Path,
) -> io::Result<PathBuf> {
    debug_assert!(is_safe(rel), "unsafe extraction path: {rel:?}");
    let comps = components(rel);
    let Some((last, parents)) = comps.split_last() else {
        return Ok(root.to_path_buf());
    };
    let parent = walk_parents(root, parents, true)?;
    let full = parent.join(last);
    match fs::symlink_metadata(&full) {
        Ok(md) if md.file_type().is_symlink() => {
            // `remove_file` handles file links everywhere; Windows needs
            // `remove_dir` for directory links.
            if fs::remove_file(&full).is_err() {
                fs::remove_dir(&full)?;
            }
        }
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    Ok(full)
}

/// Canonicalizes an existing archive-root-relative path, rejecting `..`,
/// absolute paths, and symlink escapes.
pub(crate) fn resolve_existing(
    root: &Path,
    rel: &Path,
) -> io::Result<PathBuf> {
    if !is_safe(rel) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsafe link target: {}", rel.display()),
        ));
    }
    let resolved = fs::canonicalize(root.join(rel))?;
    if !resolved.starts_with(root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("link target escapes destination: {}", rel.display()),
        ));
    }
    Ok(resolved)
}

/// True when a symlink at `link_dir` pointing at `target` stays inside
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
