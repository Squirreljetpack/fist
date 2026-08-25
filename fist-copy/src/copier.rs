use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::config::ConflictStrategy;
use super::error::WorkError;
use super::extract::runner;
use super::meta::{Attrs, apply_file_meta};
use super::reflink;
use super::scheduler::TaskEntry;
use super::work::{FileJob, LinkJob, QueuedWork, WorkItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemOutcome {
    FileOk,
    ExtractOk,
    Failed,
    Skipped,
}

pub(crate) fn execute(
    task: &TaskEntry,
    work: QueuedWork,
    scratch: &mut [u8],
) -> ItemOutcome {
    let desc = describe(&work.item);
    let res = match work.item {
        WorkItem::File(f) => copy_one(task, f, scratch),
        WorkItem::Link(l) => recreate_link(task, &l),
        WorkItem::Extract(e) => runner::run(task, &e).map(|_| ItemOutcome::ExtractOk),
    };
    match res {
        Ok(outcome) => outcome,
        Err(WorkError::Canceled) => ItemOutcome::Skipped,
        Err(WorkError::Io(e)) => {
            task.log.error(format!("{desc}: {e}"));
            ItemOutcome::Failed
        }
    }
}

fn describe(item: &WorkItem) -> String {
    match item {
        WorkItem::File(f) => format!("copy {} -> {}", f.src.display(), f.dst.display()),
        WorkItem::Link(l) => format!(
            "symlink {} -> {} (target {})",
            l.src.display(),
            l.dst.display(),
            l.target.display()
        ),
        WorkItem::Extract(e) => format!("extract {} -> {}", e.source.display(), e.dest.display()),
    }
}

/// The destination decision for one entry.
enum Resolve {
    /// Write to this path (already claimed free or declared replaceable).
    Write(PathBuf),
    /// Entry skipped by policy; the caller reports it.
    Skip,
}

/// Apply the job's [`ConflictStrategy`] against an existing destination.
/// Pure decision-making: progress accounting happens in the callers that
/// turn [`Resolve`] into an [`ItemOutcome`].
fn resolve_dest(
    task: &TaskEntry,
    dst: &Path,
) -> Result<Resolve, WorkError> {
    let job = task.active();
    let free = || Ok(Resolve::Write(dst.to_path_buf()));
    match job.params.conflict {
        ConflictStrategy::Overwrite => {
            // replacing a directory is not representable at entry level;
            // fail honestly instead of tripping over EISDIR later
            if fs::metadata(dst).map(|m| m.is_dir()).unwrap_or(false) {
                return Err(WorkError::Io(std::io::Error::new(
                    std::io::ErrorKind::IsADirectory,
                    format!(
                        "cannot overwrite: destination is a directory: {}",
                        dst.display()
                    ),
                )));
            }
            let _ = fs::remove_file(dst);
            free()
        }
        ConflictStrategy::Fail => {
            if dest_exists(dst) {
                Err(WorkError::Io(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!(
                        "destination exists (conflict strategy: fail): {}",
                        dst.display()
                    ),
                )))
            } else {
                free()
            }
        }
        ConflictStrategy::Skip => {
            if dest_exists(dst) {
                task.log.info(format!(
                    "skipping existing destination (conflict strategy: skip): {}",
                    dst.display()
                ));
                Ok(Resolve::Skip)
            } else {
                free()
            }
        }
        ConflictStrategy::RenameSuffix => match free_sibling_of(dst) {
            Some(path) => Ok(Resolve::Write(path)),
            None => Err(WorkError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("no free suffixed name for destination: {}", dst.display()),
            ))),
        },
        ConflictStrategy::Abort => {
            if dest_exists(dst) {
                task.log.info(format!(
                    "destination exists (conflict strategy: abort): {}",
                    dst.display()
                ));
                job.token.cancel();
                Err(WorkError::Canceled)
            } else {
                free()
            }
        }
    }
}

fn dest_exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

fn free_sibling_of(dst: &Path) -> Option<PathBuf> {
    if !dest_exists(dst) {
        return Some(dst.to_path_buf());
    }
    (1..=9999)
        .map(|n| with_suffix(dst, n))
        .find(|cand| !dest_exists(cand))
}

fn with_suffix(
    dst: &Path,
    n: u32,
) -> PathBuf {
    let file_name = dst
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let new_name = match (dst.file_stem(), dst.extension()) {
        (Some(stem), Some(ext)) => {
            format!("{}_{}.{}", stem.to_string_lossy(), n, ext.to_string_lossy())
        }
        (Some(stem), None) => format!("{}_{}", stem.to_string_lossy(), n),
        _ => format!("{}_{n}", file_name),
    };
    dst.with_file_name(new_name)
}

/// Returns `OpenOptions` configured to never traverse trailing symlinks or reparse points.
pub(crate) fn no_follow_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opts.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    opts
}

fn copy_one(
    task: &TaskEntry,
    f: FileJob,
    scratch: &mut [u8],
) -> Result<ItemOutcome, WorkError> {
    let job = task.active();
    if job.token.is_cancelled() {
        return Err(WorkError::Canceled);
    }
    // a root claim is a bound submit-time decision: write through it,
    // bypassing per-entry conflict resolution. Inner entries resolve.
    let FileJob {
        src,
        dst,
        len,
        attrs,
        mut bound_fd,
    } = f;
    let dst = if bound_fd.is_some() {
        dst
    } else {
        match resolve_dest(task, &dst)? {
            Resolve::Write(dst) => dst,
            Resolve::Skip => {
                task.prog.skip_file();
                return Ok(ItemOutcome::Skipped);
            }
        }
    };

    match reflink::attempt(&src, &dst, &mut bound_fd, job.params.reflink) {
        reflink::Attempt::Cloned => {
            task.prog.add_copied(len);
            return Ok(seal_copy(task, &attrs, &src, &dst));
        }
        reflink::Attempt::Buffered(Some(e)) => task.log.info(format!(
            "reflink unavailable for {} ({e}); falling back to buffered copy",
            src.display()
        )),
        reflink::Attempt::Buffered(None) => {}
    }

    // destination parents exist by submit-time resolution; inner tree
    // entries get theirs from the walk before any child is enqueued
    let mut reader = File::open(&src)?;
    // a surviving bound descriptor *is* the reservation: writing through
    // it keeps the root atomic; vacated claims and inner entries reopen
    // by path here
    let mut out = match bound_fd.take() {
        Some(file) => file,
        None => {
            let mut opts = no_follow_options();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            if job.params.preserve_metadata
                && let Some(mode) = attrs.mode
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(mode);
            }
            opts.open(&dst)?
        }
    };
    out.set_len(len)?;

    loop {
        if job.token.is_cancelled() {
            return Err(WorkError::Canceled);
        }
        let n = reader.read(scratch)?;
        if n == 0 {
            break;
        }
        out.write_all(&scratch[..n])?;
        task.prog.add_copied(n as u64);
    }
    out.flush()?;
    drop(out);

    Ok(seal_copy(task, &attrs, &src, &dst))
}

fn recreate_link(
    task: &TaskEntry,
    l: &LinkJob,
) -> Result<ItemOutcome, WorkError> {
    let job = task.active();
    if job.token.is_cancelled() {
        return Err(WorkError::Canceled);
    }
    let dst = match resolve_dest(task, &l.dst)? {
        Resolve::Write(dst) => dst,
        Resolve::Skip => {
            task.prog.skip_file();
            return Ok(ItemOutcome::Skipped);
        }
    };
    create_symlink(&l.target, &dst)?;
    task.prog.add_copied(0);
    if job.params.r#move {
        delete_source_path(task, &l.src);
    }
    Ok(ItemOutcome::FileOk)
}

pub(crate) fn create_symlink(
    target: &Path,
    dst: &Path,
) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(target, dst)
    }
    #[cfg(windows)]
    {
        let is_dir = fs::metadata(target).map(|m| m.is_dir()).unwrap_or(false);
        if is_dir {
            std::os::windows::fs::symlink_dir(target, dst)
        } else {
            std::os::windows::fs::symlink_file(target, dst)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, dst);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks not supported",
        ))
    }
}

fn delete_source_path(
    task: &TaskEntry,
    src: &Path,
) {
    match fs::remove_file(src) {
        Ok(()) => {
            task.prog.cleanup_started();
            task.prog.cleanup_done(1);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            task.log
                .info(format!("source vanished during move: {}", src.display()));
            task.prog.cleanup_started();
            task.prog.cleanup_done(1);
        }
        Err(e) => {
            task.prog.cleanup_started();
            task.prog.cleanup_failed();
            task.log
                .error(format!("could not remove source {}: {e}", src.display()));
        }
    }
}

/// Metadata and source cleanup shared by every successful copy path
/// (reflinked or buffered); byte progress is accounted by the callers.
fn seal_copy(
    task: &TaskEntry,
    attrs: &Attrs,
    src: &Path,
    dst: &Path,
) -> ItemOutcome {
    let job = task.active();
    apply_file_meta(dst, attrs, job.params.preserve_metadata);
    if job.params.r#move {
        delete_source_path(task, src);
    }
    ItemOutcome::FileOk
}
