use std::path::PathBuf;

use crate::meta::Attrs;
use crate::scheduler::TaskId;

pub(crate) type DirId = usize;

#[derive(Debug)]
pub(crate) struct FileJob {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub len: u64,
    pub attrs: Attrs,
}

#[derive(Debug)]
pub(crate) struct LinkJob {
    pub src: PathBuf,
    pub dst: PathBuf,
    pub target: PathBuf,
}

#[derive(Debug)]
pub(crate) enum WorkItem {
    File(FileJob),
    Link(LinkJob),
}

#[derive(Debug)]
pub(crate) struct QueuedWork {
    pub task: TaskId,
    pub parent: Option<DirId>,
    pub item: WorkItem,
}
