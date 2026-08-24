use std::collections::HashMap;
use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};

use crate::config::{ConflictStrategy, CopyParams, ReflinkMode};
use crate::copier::{self, ItemOutcome};
use crate::job::{JobKind, JobRequest, SubmitError};
use crate::log::TaskLog;
use crate::meta::DirMetaTracker;
use crate::progress::{Progress, TaskSnapshot, TaskState};
use crate::token::CancelToken;
use crate::walker::{self, WalkAbort};
use crate::work::QueuedWork;

pub type TaskId = u64;

#[derive(Debug, Clone)]
pub struct SchedulerOptions {
    pub workers: NonZeroUsize,
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self {
            workers: CopyParams::default().workers,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskHandle {
    pub id: TaskId,
    token: CancelToken,
    prog: Arc<Progress>,
    log: Arc<TaskLog>,
}

impl TaskHandle {
    pub fn cancel(&self) {
        self.token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub fn snapshot(&self) -> TaskSnapshot {
        self.prog.snapshot()
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.log.lines()
    }
}

pub(crate) struct JobCtx {
    pub id: TaskId,
    pub source: PathBuf,
    pub dest: PathBuf,
    pub delete_source: bool,
    pub preserve_metadata: bool,
    pub reflink_mode: ReflinkMode,
    #[allow(dead_code)]
    pub conflict: ConflictStrategy,
    pub buffer_size: usize,
    /// Extraction jobs run one archive-level work item instead of a walk.
    pub is_extract: bool,
    /// Directory-transfer behavior when the target directory exists.
    pub merge: crate::config::MergeStrategy,
    pub token: CancelToken,
    pub prog: Arc<Progress>,
    pub log: Arc<TaskLog>,
    pub tracker: DirMetaTracker,
    outstanding: AtomicUsize,
    walk_done: AtomicBool,
}

impl JobCtx {
    pub(crate) fn enqueue(
        &self,
        tx: &Sender<QueuedWork>,
        parent: Option<usize>,
        item: crate::work::WorkItem,
    ) -> Result<(), crossbeam_channel::SendError<QueuedWork>> {
        self.outstanding.fetch_add(1, Ordering::AcqRel);
        let w = QueuedWork {
            task: self.id,
            parent,
            item,
        };
        if let Err(e) = tx.send(w) {
            self.outstanding.fetch_sub(1, Ordering::AcqRel);
            Err(e)
        } else {
            Ok(())
        }
    }

    fn complete_item(
        self: &Arc<Self>,
        parent: Option<usize>,
        outcome: ItemOutcome,
        count_files: bool,
    ) {
        match outcome {
            ItemOutcome::Done if count_files => self.prog.file_ok(),
            ItemOutcome::Failed if count_files => self.prog.file_failed(),
            _ => {}
        }
        if let Some(p) = parent {
            self.tracker.child_finished(p);
        }
        if self.outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.try_finish();
        }
    }

    fn try_finish(self: &Arc<Self>) {
        if !self.walk_done.load(Ordering::Acquire) {
            return;
        }
        if self.tracker.has_dirs() && !self.tracker.root_finished() {
            return;
        }
        if self.token.is_cancelled() {
            let _ = self.prog.cas_state(TaskState::Started, TaskState::Canceled);
            return;
        }
        if self.delete_source && self.prog.snapshot().files_failed == 0 {
            self.prog.cleanup_force_success();
        }
        let failed = self.prog.snapshot().files_failed > 0;
        let target = if failed {
            TaskState::CompleteErr
        } else {
            TaskState::CompleteOk
        };
        if self.prog.cas_state(TaskState::Started, target).is_ok() {
            let s = self.prog.snapshot();
            self.log.info(format!(
                "finished {}: {} ok, {} failed{}, {} bytes",
                if failed { "with errors" } else { "ok" },
                s.files_ok,
                s.files_failed,
                if s.files_skipped > 0 {
                    format!(", {} skipped", s.files_skipped)
                } else {
                    String::new()
                },
                s.copied_bytes
            ));
        }
    }
}

struct Inner {
    shutting_down: AtomicBool,
    next_id: AtomicU64,
    jobs: Mutex<HashMap<TaskId, Arc<JobCtx>>>,
    tx: Mutex<Option<Sender<QueuedWork>>>,
    rx: Receiver<QueuedWork>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    collectors: Mutex<Vec<JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<Inner>,
}

impl Scheduler {
    pub fn new(opts: SchedulerOptions) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let inner = Arc::new(Inner {
            shutting_down: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            jobs: Mutex::new(HashMap::new()),
            tx: Mutex::new(Some(tx)),
            rx,
            workers: Mutex::new(Vec::with_capacity(opts.workers.get())),
            collectors: Mutex::new(Vec::new()),
        });
        for i in 0..opts.workers.get() {
            let h = spawn_worker(inner.clone(), i);
            inner.workers.lock().expect("workers lock").push(h);
        }
        Scheduler { inner }
    }

    pub fn submit(
        &self,
        req: JobRequest,
    ) -> Result<TaskHandle, SubmitError> {
        if self.inner.shutting_down.load(Ordering::Acquire) {
            return Err(SubmitError::ShuttingDown);
        }
        if fs::symlink_metadata(&req.source).is_err() {
            return Err(SubmitError::SourceMissing(req.source.clone()));
        }
        let is_extract = matches!(req.kind, JobKind::Extract(_));
        if !is_extract && req.dest.starts_with(&req.source) {
            return Err(SubmitError::IntoItself {
                source: req.source.clone(),
                dest: req.dest.clone(),
            });
        }

        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (params, is_move, delete_source): (&CopyParams, bool, bool) = match &req.kind {
            JobKind::Copy(p) => (p, false, false),
            JobKind::Move(m) => (&m.copy, true, m.delete_source),
            JobKind::Extract(_) => (&CopyParams::default(), false, false),
        };
        let prog = Arc::new(Progress::new(is_move && delete_source));
        let log = Arc::new(TaskLog::default());
        log.info(format!(
            "task {id}: {} {} -> {}",
            kind_name(is_move, is_extract),
            req.source.display(),
            req.dest.display()
        ));
        let tracker = DirMetaTracker::new(
            params.preserve_metadata,
            is_move && delete_source,
            prog.clone(),
            log.clone(),
        );
        let job = Arc::new(JobCtx {
            id,
            source: req.source.clone(),
            dest: req.dest.clone(),
            delete_source: is_move && delete_source,
            preserve_metadata: params.preserve_metadata,
            reflink_mode: params.reflink,
            conflict: params.conflict,
            buffer_size: params.buffer_size.get(),
            is_extract,
            merge: params.merge,
            token: CancelToken::new(),
            prog,
            log,
            tracker,
            outstanding: AtomicUsize::new(1),
            walk_done: AtomicBool::new(false),
        });

        if is_move && delete_source && fast_rename(&job) {
            self.inner
                .jobs
                .lock()
                .expect("jobs lock")
                .insert(id, job.clone());
            return Ok(handle_of(&job));
        }

        self.inner
            .jobs
            .lock()
            .expect("jobs lock")
            .insert(id, job.clone());

        let h = spawn_collector(self.inner.clone(), job.clone());
        self.inner
            .collectors
            .lock()
            .expect("collectors lock")
            .push(h);

        Ok(handle_of(&job))
    }

    pub fn cancel(
        &self,
        id: TaskId,
    ) {
        if let Some(job) = self.inner.jobs.lock().expect("jobs lock").get(&id) {
            job.token.cancel();
        }
    }

    pub fn cancel_all(&self) {
        for job in self.inner.jobs.lock().expect("jobs lock").values() {
            job.token.cancel();
        }
    }

    pub fn snapshot(&self) -> Vec<(TaskId, TaskSnapshot)> {
        self.inner
            .jobs
            .lock()
            .expect("jobs lock")
            .iter()
            .map(|(id, j)| (*id, j.prog.snapshot()))
            .collect()
    }

    /// Log lines recorded so far for one task. Tasks are never evicted, so
    /// this stays readable after completion.
    pub fn log_lines(
        &self,
        id: TaskId,
    ) -> Option<Vec<String>> {
        self.inner
            .jobs
            .lock()
            .expect("jobs lock")
            .get(&id)
            .map(|j| j.log.lines())
    }

    pub fn shutdown(
        &self,
        drain_timeout: Duration,
    ) {
        self.inner.shutting_down.store(true, Ordering::Release);
        self.cancel_all();
        let deadline = Instant::now() + drain_timeout;
        join_before(
            self.inner
                .collectors
                .lock()
                .expect("collectors lock")
                .drain(..),
            deadline,
        );
        *self.inner.tx.lock().expect("tx lock") = None;
        join_before(
            self.inner.workers.lock().expect("workers lock").drain(..),
            deadline,
        );
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown(Duration::from_secs(3));
    }
}

fn handle_of(job: &Arc<JobCtx>) -> TaskHandle {
    TaskHandle {
        id: job.id,
        token: job.token.clone(),
        prog: job.prog.clone(),
        log: job.log.clone(),
    }
}

fn kind_name(
    is_move: bool,
    is_extract: bool,
) -> &'static str {
    if is_extract {
        "extract"
    } else if is_move {
        "move"
    } else {
        "copy"
    }
}

fn fast_rename(job: &Arc<JobCtx>) -> bool {
    match fs::rename(&job.source, &job.dest) {
        Ok(()) => {
            if let Ok(md) = fs::symlink_metadata(&job.dest) {
                if md.is_dir() {
                    job.prog.register_file(0);
                } else {
                    job.prog.register_file(md.len());
                    job.prog.add_copied(md.len());
                }
            }
            job.prog.cleanup_skip();
            job.prog.file_ok();
            let _ = job
                .prog
                .cas_state(TaskState::Pending, TaskState::CompleteOk);
            job.log.info("completed via same-filesystem rename");
            true
        }
        Err(e) => {
            if crate::error::is_cross_device(&e) || e.kind() != std::io::ErrorKind::NotFound {
                job.log.info(format!(
                    "rename fast path unavailable ({e}); falling back to full copy"
                ));
            }
            false
        }
    }
}

fn spawn_worker(
    inner: Arc<Inner>,
    idx: usize,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("fist-copy-worker-{idx}"))
        .spawn(move || worker_loop(inner))
        .expect("spawn copy worker")
}

fn worker_loop(inner: Arc<Inner>) {
    let mut scratch: Vec<u8> = Vec::new();
    loop {
        match inner.rx.recv_timeout(Duration::from_millis(50)) {
            Ok(w) => {
                let job = { inner.jobs.lock().expect("jobs lock").get(&w.task).cloned() };
                let Some(job) = job else { continue };
                if job.token.is_cancelled() {
                    job.complete_item(w.parent, ItemOutcome::Skipped, false);
                    continue;
                }
                if scratch.len() < job.buffer_size {
                    scratch.resize(job.buffer_size, 0);
                }
                let outcome = copier::execute(&job, &w, &mut scratch[..]);
                // extraction items do their own per-entry accounting inside
                // the runner; counting the archive-level item again would
                // inflate files_ok/files_failed by one
                let self_counting = matches!(w.item, crate::work::WorkItem::Extract(_));
                job.complete_item(w.parent, outcome, !self_counting);
            }
            Err(RecvTimeoutError::Timeout) => {
                if inner.shutting_down.load(Ordering::Acquire) {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn spawn_collector(
    inner: Arc<Inner>,
    job: Arc<JobCtx>,
) -> JoinHandle<()> {
    let tx = inner
        .tx
        .lock()
        .expect("tx lock")
        .as_ref()
        .expect("sender present while scheduler is live")
        .clone();
    thread::Builder::new()
        .name("fist-copy-walk".to_string())
        .spawn(move || collector_run(job, tx))
        .expect("spawn copy walker")
}

fn collector_run(
    job: Arc<JobCtx>,
    tx: Sender<QueuedWork>,
) {
    if job.token.is_cancelled() {
        let _ = job.prog.cas_state(TaskState::Pending, TaskState::Canceled);
    } else {
        let _ = job.prog.cas_state(TaskState::Pending, TaskState::Started);
        let collect_result = if job.is_extract {
            walker::collect_extract(&job, &tx)
        } else {
            walker::collect(&job, &tx)
        };
        match collect_result {
            Ok(()) => {}
            Err(WalkAbort::Canceled) => {
                let _ = job.prog.cas_state(TaskState::Started, TaskState::Canceled);
            }
            Err(WalkAbort::IntoItself) => {
                job.log.error(format!(
                    "cannot move/copy directory into itself: {} -> {}",
                    job.source.display(),
                    job.dest.display()
                ));
                let _ = job
                    .prog
                    .cas_state(TaskState::Started, TaskState::CompleteErr);
            }
            Err(WalkAbort::Io(e)) => {
                job.log
                    .error(format!("walk of {} failed: {e}", job.source.display()));
                let _ = job
                    .prog
                    .cas_state(TaskState::Started, TaskState::CompleteErr);
            }
        }
    }
    job.walk_done.store(true, Ordering::Release);
    if job.outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
        job.try_finish();
    }
}

fn join_before(
    handles: impl Iterator<Item = JoinHandle<()>>,
    deadline: Instant,
) {
    for h in handles {
        while !h.is_finished() {
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_millis(2));
        }
        let _ = h.join();
    }
}
