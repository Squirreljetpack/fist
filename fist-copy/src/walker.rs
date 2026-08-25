use std::fs;
use std::io;
use std::path::Path;

use walkdir::WalkDir;

use super::meta::Attrs;
use super::scheduler::{CollectorPlan, RootClaim, TaskEntry};
use super::work::{FileJob, LinkJob, QueuedWork, WorkItem};

pub(crate) enum WalkAbort {
    Canceled,
    Io(io::Error),
}

impl From<crossbeam_channel::SendError<QueuedWork>> for WalkAbort {
    fn from(_: crossbeam_channel::SendError<QueuedWork>) -> Self {
        WalkAbort::Io(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "worker pool unavailable",
        ))
    }
}

/// The off-thread collection phase: consumes the submit-time claim, prepares
/// the destination, and enqueues individual work items for the worker pool.
pub(crate) fn collect(
    task: &TaskEntry,
    plan: CollectorPlan,
    tx: &crossbeam_channel::Sender<QueuedWork>,
) -> Result<(), WalkAbort> {
    let is_move = task.active().params.r#move;
    match plan.claim {
        RootClaim::File(claim) => {
            let (dst, fd) = claim.into_parts().ok_or_else(|| {
                WalkAbort::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    "claimed file placeholder was invalidated",
                ))
            })?;
            task.log.info(format!(
                "{} {} -> {}",
                if is_move { "move" } else { "copy" },
                plan.source.display(),
                dst.display()
            ));
            let smeta = fs::symlink_metadata(&plan.source).map_err(WalkAbort::Io)?;
            task.prog.register_file(smeta.len());
            task.enqueue(
                tx,
                None,
                WorkItem::File(FileJob {
                    src: plan.source,
                    dst,
                    len: smeta.len(),
                    attrs: Attrs::from_metadata(&smeta),
                    bound_fd: Some(fd),
                }),
            )?;
            Ok(())
        }
        RootClaim::Dir(claim) => {
            let dst_root = claim.into_path().ok_or_else(|| {
                WalkAbort::Io(io::Error::new(
                    io::ErrorKind::DirectoryNotEmpty,
                    "claimed directory was populated or invalidated",
                ))
            })?;
            task.log.info(format!(
                "{} {} -> {}",
                if is_move { "move" } else { "copy" },
                plan.source.display(),
                dst_root.display()
            ));
            if plan.clear_dest {
                clear_dir_contents(&dst_root).map_err(WalkAbort::Io)?;
            }

            let smeta = fs::symlink_metadata(&plan.source).map_err(WalkAbort::Io)?;
            task.active().tracker.register_root(
                dst_root.clone(),
                plan.source.clone(),
                Attrs::from_metadata(&smeta),
            );

            walk_tree(task, &plan.source, &dst_root, tx)
        }
    }
}

/// Walks the source tree depth-first, creating destination subdirectories,
/// queueing file/link work items, and ordering directory metadata seals.
fn walk_tree(
    task: &TaskEntry,
    src: &Path,
    dst_root: &Path,
    tx: &crossbeam_channel::Sender<QueuedWork>,
) -> Result<(), WalkAbort> {
    const ROOT: usize = 0;
    let mut open: Vec<(usize, usize)> = vec![(ROOT, 0)];

    for entry in WalkDir::new(src).follow_links(false) {
        if task.active().token.is_cancelled() {
            return Err(WalkAbort::Canceled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                report_walk_error(task, src, &e);
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }

        // Seal directories whose subtree traversal is complete
        while open.last().expect("root always present").1 >= entry.depth() {
            let (id, _) = open.pop().expect("checked");
            task.seal_dir(id);
        }
        let cur = open.last().expect("root always present").0;

        let path = entry.path();
        let rel = path
            .strip_prefix(src)
            .map_err(|e| WalkAbort::Io(io::Error::other(e)))?;
        let dst = dst_root.join(rel);
        let ft = entry.file_type();

        if ft.is_dir() {
            let md = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    report_item_error(task, path, &e);
                    continue;
                }
            };
            // Create destination directory if needed, preventing symlink traversal
            let is_existing_dir = fs::symlink_metadata(&dst)
                .map(|m| m.is_dir())
                .unwrap_or(false);
            if !is_existing_dir {
                if let Err(e) = fs::create_dir(&dst) {
                    if !(e.kind() == io::ErrorKind::AlreadyExists && is_existing_dir) {
                        report_item_error(task, &dst, &e);
                        continue;
                    }
                }
            }
            let id = task.active().tracker.register_dir(
                cur,
                dst,
                path.to_path_buf(),
                Attrs::from_metadata(&md),
            );
            task.active().tracker.expect_child(cur);
            open.push((id, entry.depth()));
        } else if ft.is_symlink() {
            let target = match fs::read_link(path) {
                Ok(t) => t,
                Err(e) => {
                    report_item_error(task, path, &e);
                    continue;
                }
            };
            task.active().tracker.expect_child(cur);
            task.prog.register_file(0);
            task.enqueue(
                tx,
                Some(cur),
                WorkItem::Link(LinkJob {
                    src: path.to_path_buf(),
                    dst,
                    target,
                }),
            )?;
        } else {
            let md = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    report_item_error(task, path, &e);
                    continue;
                }
            };
            task.active().tracker.expect_child(cur);
            task.prog.register_file(md.len());
            task.enqueue(
                tx,
                Some(cur),
                WorkItem::File(FileJob {
                    src: path.to_path_buf(),
                    dst,
                    len: md.len(),
                    attrs: Attrs::from_metadata(&md),
                    bound_fd: None,
                }),
            )?;
        }
    }

    // Seal all remaining open ancestor directories up to root
    while let Some((id, _)) = open.pop() {
        task.seal_dir(id);
    }
    Ok(())
}

fn report_walk_error(
    task: &TaskEntry,
    root: &Path,
    err: &walkdir::Error,
) {
    task.prog.register_file(0);
    task.prog.file_failed();
    task.log
        .error(format!("walk error under {}: {err}", root.display()));
}

fn report_item_error(
    task: &TaskEntry,
    path: &Path,
    err: &dyn std::fmt::Display,
) {
    task.prog.register_file(0);
    task.prog.file_failed();
    task.log
        .error(format!("could not access {}: {err}", path.display()));
}

/// Removes the *contents* of `dir`, keeping the directory inode itself so
/// its name never vacates during an Overwrite. Symlinked children are
/// removed as links, never followed. Tolerates concurrent deletions.
pub(crate) fn clear_dir_contents(dir: &std::path::Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let child = entry.path();
        let md = match std::fs::symlink_metadata(&child) {
            Ok(md) => md,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        let res = if md.file_type().is_symlink() {
            std::fs::remove_file(&child)
        } else if md.is_dir() {
            std::fs::remove_dir_all(&child)
        } else {
            std::fs::remove_file(&child)
        };
        if let Err(e) = res
            && e.kind() != io::ErrorKind::NotFound
        {
            return Err(e);
        }
    }
    Ok(())
}
