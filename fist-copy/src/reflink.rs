use std::fs::{self, File};
use std::io;
use std::num::NonZeroU64;
use std::path::Path;

use super::config::ReflinkMode;

#[cfg(unix)]
fn same_device(
    src: &Path,
    dst_parent: &Path,
) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::symlink_metadata(src), fs::metadata(dst_parent)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn same_device(
    _src: &Path,
    _dst_parent: &Path,
) -> bool {
    false
}

/// Copies one file with block cloning where the platform allows it,
/// choosing the route from what the caller holds:
///
/// - a bound descriptor clones through itself (`FICLONE`-family ioctls),
///   keeping a submit-time claim intact because the leaf is never
///   recreated; platforms without an fd-targeted API (macOS) fall back to
///   cloning by name, vacating the placeholder first;
/// - otherwise the free destination path is cloned directly.
///
/// After [`Attempt::Buffered`] the caller buffers the ordinary way:
/// `bound_fd` still holds a usable descriptor when one survived, and is
/// `None` once the destination must be recreated by path.
pub(crate) fn attempt(
    src: &Path,
    dst: &Path,
    bound_fd: &mut Option<File>,
    mode: ReflinkMode,
) -> Attempt {
    if mode != ReflinkMode::Auto {
        return Attempt::Buffered(None);
    }
    let parent = match dst.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    if !same_device(src, parent) {
        return Attempt::Buffered(None);
    }
    if let Some(file) = bound_fd.as_ref() {
        match clone_into_fd(src, file) {
            Ok(()) => return Attempt::Cloned,
            Err(e) => {
                if !fd_clone_supported() {
                    // no fd-targeted API here (macOS clones by name):
                    // vacate the claimed name for a path-based clone;
                    // dropping the descriptor sends the caller's tail to
                    // recreate the leaf
                    drop(bound_fd.take());
                    let _ = fs::remove_file(dst);
                    return finish_by_path(src, dst);
                }
                return Attempt::Buffered(Some(e));
            }
        }
    }
    finish_by_path(src, dst)
}

/// What [`attempt`] decided for the file.
pub(crate) enum Attempt {
    /// The destination holds a clone; nothing further to write.
    Cloned,
    /// Nothing was cloned: buffer the ordinary way. `bound_fd` says
    /// whether a descriptor survived (write through it) or the leaf must
    /// be recreated by path. Carries why, for diagnostics.
    Buffered(Option<io::Error>),
}

fn finish_by_path(
    src: &Path,
    dst: &Path,
) -> Attempt {
    match clone_file(src, dst) {
        Ok(()) => Attempt::Cloned,
        Err(e) => {
            let _ = fs::remove_file(dst);
            Attempt::Buffered(Some(e))
        }
    }
}

/// Copies `src` to the free path `dst` via the platform's block-cloning
/// syscall: FICLONE on Linux, clonefile on macOS, ReFS block cloning on
/// Windows.
fn clone_file(
    src: &Path,
    dst: &Path,
) -> io::Result<()> {
    reflink_copy::reflink(src, dst)
}

/// Whether [`clone_into_fd`] is implemented on this platform; macOS clones
/// by name only and has no fd-targeted API.
fn fd_clone_supported() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows"))
}

/// Clones `src` into an already-held destination descriptor. The leaf is
/// never recreated, so a submit-time claim survives the copy.
///
/// On failure the descriptor is reset to zero-length so rollback keeps
/// treating the leaf as an untouched placeholder.
fn clone_into_fd(
    src: &Path,
    dst: &File,
) -> io::Result<()> {
    let len = fs::metadata(src)?.len();
    if len == 0 {
        // the held placeholder is already empty
        return Ok(());
    }
    // the block-clone call does not size the destination itself
    dst.set_len(len)?;
    let opened = File::open(src)?;
    let result = reflink_copy::ReflinkBlockBuilder::new(
        &opened,
        dst,
        NonZeroU64::new(len).expect("non-zero length checked above"),
    )
    .from_offset(0)
    .to_offset(0)
    .reflink_block();
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = dst.set_len(0);
            Err(e)
        }
    }
}
