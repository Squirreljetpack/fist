use std::path::PathBuf;

use super::meta::Attrs;
use super::scheduler::TaskId;

pub(crate) type DirId = usize;

#[derive(Debug)]
pub(crate) struct FileJob {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub len: u64,
    pub attrs: Attrs,
    /// Pre-opened file descriptor for single-file root transfers;
    /// `None` for inner tree entries which resolve conflicts per-entry.
    pub bound_fd: Option<std::fs::File>,
}

#[derive(Debug)]
pub(crate) struct LinkJob {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub target: PathBuf,
}

/// One archive extraction: `source` is unpacked into the existing
/// directory `dest`.
#[derive(Debug)]
pub(crate) struct ExtractJob {
    pub source: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug)]
pub(crate) enum WorkItem {
    File(FileJob),
    Link(LinkJob),
    Extract(ExtractJob),
}

#[derive(Debug)]
pub(crate) struct QueuedWork {
    pub task: TaskId,
    pub parent: Option<DirId>,
    pub item: WorkItem,
}
