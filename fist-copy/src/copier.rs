use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::{ConflictStrategy, ReflinkMode};
use crate::error::WorkError;
use crate::meta::apply_file_meta;
use crate::reflink;
use crate::scheduler::JobCtx;
use crate::work::{FileJob, LinkJob, QueuedWork, WorkItem};

pub(crate) enum ItemOutcome {
    Done,
    Failed,
    Skipped,
}

pub(crate) fn execute(
    job: &Arc<JobCtx>,
    work: &QueuedWork,
    scratch: &mut [u8],
) -> ItemOutcome {
    let res = match &work.item {
        WorkItem::File(f) => copy_one(job, f, scratch),
        WorkItem::Link(l) => recreate_link(job, l),
        WorkItem::Extract(e) => crate::extract::runner::run(job, e).map(|_| ItemOutcome::Done),
    };
    match res {
        Ok(outcome) => outcome,
        Err(WorkError::Canceled) => ItemOutcome::Skipped,
        Err(WorkError::Io(e)) => {
            job.log.error(format!("{}: {e}", describe(work)));
            ItemOutcome::Failed
        }
    }
}

fn describe(work: &QueuedWork) -> String {
    match &work.item {
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
    job: &Arc<JobCtx>,
    dst: &Path,
) -> Result<Resolve, WorkError> {
    let free = || Ok(Resolve::Write(dst.to_path_buf()));
    match job.conflict {
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
                job.log.info(format!(
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
                job.log.info(format!(
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

fn copy_one(
    job: &Arc<JobCtx>,
    f: &FileJob,
    scratch: &mut [u8],
) -> Result<ItemOutcome, WorkError> {
    if job.token.is_cancelled() {
        return Err(WorkError::Canceled);
    }
    let dst = match resolve_dest(job, &f.dst)? {
        Resolve::Write(dst) => dst,
        Resolve::Skip => {
            job.prog.skip_file();
            return Ok(ItemOutcome::Skipped);
        }
    };

    if job.reflink_mode == ReflinkMode::Auto && reflink::same_device(&f.src, parent_of(&dst)) {
        match reflink::clone_file(&f.src, &dst) {
            Ok(()) => {
                apply_file_meta(&dst, &f.attrs, job.preserve_metadata);
                job.prog.add_copied(f.len);
                if job.delete_source {
                    delete_source_path(job, &f.src);
                }
                return Ok(ItemOutcome::Done);
            }
            Err(e) => {
                job.log.info(format!(
                    "reflink unavailable for {} ({e}); falling back to buffered copy",
                    f.src.display()
                ));
                let _ = fs::remove_file(&dst);
            }
        }
    }

    ensure_parent(&dst)?;
    let mut src = File::open(&f.src)?;
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if job.preserve_metadata
        && let Some(mode) = f.attrs.mode
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(mode);
    }
    let mut out = opts.open(&dst)?;
    out.set_len(f.len)?;

    loop {
        if job.token.is_cancelled() {
            return Err(WorkError::Canceled);
        }
        let n = src.read(scratch)?;
        if n == 0 {
            break;
        }
        out.write_all(&scratch[..n])?;
        job.prog.add_copied(n as u64);
    }
    out.flush()?;
    drop(out);

    apply_file_meta(&dst, &f.attrs, job.preserve_metadata);
    if job.delete_source {
        delete_source_path(job, &f.src);
    }
    Ok(ItemOutcome::Done)
}

fn recreate_link(
    job: &Arc<JobCtx>,
    l: &LinkJob,
) -> Result<ItemOutcome, WorkError> {
    if job.token.is_cancelled() {
        return Err(WorkError::Canceled);
    }
    let dst = match resolve_dest(job, &l.dst)? {
        Resolve::Write(dst) => dst,
        Resolve::Skip => {
            job.prog.skip_file();
            return Ok(ItemOutcome::Skipped);
        }
    };
    create_symlink(&l.target, &dst)?;
    job.prog.add_copied(0);
    if job.delete_source {
        delete_source_path(job, &l.src);
    }
    Ok(ItemOutcome::Done)
}

fn create_symlink(
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
    job: &Arc<JobCtx>,
    src: &Path,
) {
    match fs::remove_file(src) {
        Ok(()) => {
            job.prog.cleanup_started();
            job.prog.cleanup_done(1);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            job.log
                .info(format!("source vanished during move: {}", src.display()));
            job.prog.cleanup_started();
            job.prog.cleanup_done(1);
        }
        Err(e) => {
            job.prog.cleanup_started();
            job.prog.cleanup_failed();
            job.log
                .error(format!("could not remove source {}: {e}", src.display()));
        }
    }
}

fn ensure_parent(dst: &Path) -> std::io::Result<()> {
    if let Some(p) = dst.parent()
        && !p.as_os_str().is_empty()
    {
        fs::create_dir_all(p)?;
    }
    Ok(())
}

fn parent_of(p: &Path) -> &Path {
    match p.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}
