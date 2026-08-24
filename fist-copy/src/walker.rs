use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use walkdir::WalkDir;

use crate::meta::Attrs;
use crate::scheduler::JobCtx;
use crate::work::{FileJob, LinkJob, QueuedWork, WorkItem};

pub(crate) enum WalkAbort {
    Canceled,
    IntoItself,
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

pub(crate) fn collect(
    job: &Arc<JobCtx>,
    tx: &crossbeam_channel::Sender<QueuedWork>,
) -> Result<(), WalkAbort> {
    let src = job.source.clone();
    let dst_root = job.dest.clone();

    let smeta = fs::symlink_metadata(&src).map_err(WalkAbort::Io)?;

    if !smeta.is_dir() {
        return collect_single(job, &src, &dst_root, &smeta, tx);
    }

    if dst_root.starts_with(&src) {
        return Err(WalkAbort::IntoItself);
    }

    fs::create_dir_all(&dst_root).map_err(WalkAbort::Io)?;
    job.tracker
        .register_root(dst_root.clone(), src.clone(), Attrs::from_metadata(&smeta));

    const ROOT: usize = 0;
    let mut open: Vec<(usize, usize)> = vec![(ROOT, 0)];

    for entry in WalkDir::new(&src).follow_links(false) {
        if job.token.is_cancelled() {
            return Err(WalkAbort::Canceled);
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                job.prog.register_file(0);
                job.prog.file_failed();
                job.log
                    .error(format!("walk error under {}: {e}", src.display()));
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }

        while open.last().expect("root always present").1 >= entry.depth() {
            let (id, _) = open.pop().expect("checked");
            job.tracker.seal(id);
        }
        let cur = open.last().expect("root always present").0;

        let path = entry.path();
        let rel = path
            .strip_prefix(&src)
            .map_err(|e| WalkAbort::Io(io::Error::other(e)))?;
        let dst = dst_root.join(rel);
        let ft = entry.file_type();

        if ft.is_dir() {
            let md = match entry.metadata() {
                Ok(m) => m,
                Err(e) => {
                    job.prog.register_file(0);
                    job.prog.file_failed();
                    job.log
                        .error(format!("could not stat {}: {e}", path.display()));
                    continue;
                }
            };
            if let Err(e) = fs::create_dir(&dst)
                && !(e.kind() == io::ErrorKind::AlreadyExists && dst.is_dir())
            {
                job.prog.register_file(0);
                job.prog.file_failed();
                job.log
                    .error(format!("could not create directory {}: {e}", dst.display()));
                continue;
            }
            let id =
                job.tracker
                    .register_dir(cur, dst, path.to_path_buf(), Attrs::from_metadata(&md));
            job.tracker.expect_child(cur);
            open.push((id, entry.depth()));
        } else if ft.is_symlink() {
            let target = match fs::read_link(path) {
                Ok(t) => t,
                Err(e) => {
                    job.prog.register_file(0);
                    job.prog.file_failed();
                    job.log
                        .error(format!("could not read symlink {}: {e}", path.display()));
                    continue;
                }
            };
            job.tracker.expect_child(cur);
            job.prog.register_file(0);
            job.enqueue(
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
                    job.prog.register_file(0);
                    job.prog.file_failed();
                    job.log
                        .error(format!("could not stat {}: {e}", path.display()));
                    continue;
                }
            };
            job.tracker.expect_child(cur);
            job.prog.register_file(md.len());
            job.enqueue(
                tx,
                Some(cur),
                WorkItem::File(FileJob {
                    src: path.to_path_buf(),
                    dst,
                    len: md.len(),
                    attrs: Attrs::from_metadata(&md),
                }),
            )?;
        }
    }

    while let Some((id, _)) = open.pop() {
        job.tracker.seal(id);
    }
    Ok(())
}

/// Collection phase for extraction jobs: enqueues the single archive-level
/// work item. Entry registration happens inside the extraction loop.
pub(crate) fn collect_extract(
    job: &Arc<JobCtx>,
    tx: &crossbeam_channel::Sender<QueuedWork>,
) -> Result<(), WalkAbort> {
    job.enqueue(
        tx,
        None,
        WorkItem::Extract(crate::work::ExtractJob {
            source: job.source.clone(),
            dest: job.dest.clone(),
        }),
    )?;
    Ok(())
}

fn collect_single(
    job: &Arc<JobCtx>,
    src: &Path,
    dst: &Path,
    smeta: &fs::Metadata,
    tx: &crossbeam_channel::Sender<QueuedWork>,
) -> Result<(), WalkAbort> {
    if dst.starts_with(src) {
        return Err(WalkAbort::IntoItself);
    }
    if let Some(parent) = dst.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(WalkAbort::Io)?;
    }
    if smeta.file_type().is_symlink() {
        let target = fs::read_link(src).map_err(WalkAbort::Io)?;
        job.prog.register_file(0);
        job.enqueue(
            tx,
            None,
            WorkItem::Link(LinkJob {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
                target,
            }),
        )?;
        Ok(())
    } else {
        job.prog.register_file(smeta.len());
        job.enqueue(
            tx,
            None,
            WorkItem::File(FileJob {
                src: src.to_path_buf(),
                dst: dst.to_path_buf(),
                len: smeta.len(),
                attrs: Attrs::from_metadata(smeta),
            }),
        )?;
        Ok(())
    }
}
