use std::collections::HashMap;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender};

use super::config::{RootStrategy, TransferParams};
use super::copier::{self, ItemOutcome};
use super::job::{JobKind, JobRequest, SubmitError};
use super::log::TaskLog;
use super::meta::{DirContext, DirMetaTracker};
use super::progress::{Progress, TaskSnapshot, TaskState};
use super::token::CancelToken;
use super::walker::{self, WalkAbort};
use super::work::{DirId, ExtractJob, QueuedWork, WorkItem};

pub type TaskId = u64;

#[derive(Debug, Clone)]
pub struct SchedulerOptions {
    pub workers: NonZeroUsize,
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self {
            workers: TransferParams::default().workers,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskHandle {
    pub id: TaskId,
    /// Present while the task is engine-backed. Synchronous completions
    /// carry none: their end state is already reached, so there is
    /// nothing to cancel.
    token: Option<CancelToken>,
    prog: Arc<Progress>,
    log: Arc<TaskLog>,
}

impl TaskHandle {
    pub fn cancel(&self) {
        if let Some(token) = &self.token {
            token.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.token.as_ref().is_some_and(|t| t.is_cancelled())
    }

    pub fn snapshot(&self) -> TaskSnapshot {
        self.prog.snapshot()
    }

    pub fn log_lines(&self) -> Vec<String> {
        self.log.lines()
    }
}

use cba::claim::{self as cba_claim, ClaimError, DirClaim, FileClaim};

pub(crate) enum RootClaim {
    File(FileClaim),
    Dir(DirClaim),
}

/// Per-task instructions for the collector phase, produced by submit-time
/// root resolution and consumed exactly once. Plain owned values: thread
/// spawn provides the ordering, so no shared-state plumbing is needed.
pub(crate) struct CollectorPlan {
    /// Root source path; consumed by the collector thread.
    pub(crate) source: PathBuf,
    /// Root claim (file or directory); consumed and logged in the collector.
    pub(crate) claim: RootClaim,
    /// Whether the destination directory contents must be cleared prior to copying.
    pub(crate) clear_dest: bool,
}

/// One registered task. `job` is `None` for tasks that reached their end
/// state synchronously at submission time (fast-path moves, identity and
/// skip outcomes); those are terminal by construction, so no cancel token
/// exists for them.
pub(crate) struct TaskEntry {
    pub(crate) id: TaskId,
    pub(crate) prog: Arc<Progress>,
    pub(crate) log: Arc<TaskLog>,
    pub(crate) job: Option<JobCtx>,
}

impl TaskEntry {
    /// The running job of this task. Collector and worker paths only ever
    /// handle registered active tasks.
    pub(crate) fn active(&self) -> &JobCtx {
        self.job.as_ref().expect("active task carries its job")
    }

    /// Cancels the live job; synchronous completions have nothing left to
    /// cancel.
    pub(crate) fn cancel(&self) {
        if let Some(job) = &self.job {
            job.token.cancel();
        }
    }

    pub(crate) fn handle(&self) -> TaskHandle {
        TaskHandle {
            id: self.id,
            token: self.job.as_ref().map(|j| j.token.clone()),
            prog: self.prog.clone(),
            log: self.log.clone(),
        }
    }

    pub(crate) fn enqueue(
        &self,
        tx: &Sender<QueuedWork>,
        parent: Option<usize>,
        item: WorkItem,
    ) -> Result<(), crossbeam_channel::SendError<QueuedWork>> {
        let job = self.active();
        job.outstanding.fetch_add(1, Ordering::AcqRel);
        let w = QueuedWork {
            task: self.id,
            parent,
            item,
        };
        if let Err(e) = tx.send(w) {
            job.outstanding.fetch_sub(1, Ordering::AcqRel);
            Err(e)
        } else {
            Ok(())
        }
    }

    pub(crate) fn dir_context(&self) -> DirContext<'_> {
        let job = self.active();
        DirContext {
            prog: &self.prog,
            log: &self.log,
            preserve_meta: job.params.preserve_metadata,
            is_move: job.params.r#move,
        }
    }

    pub(crate) fn seal_dir(
        &self,
        dir: DirId,
    ) {
        self.active().tracker.seal(dir, self.dir_context());
    }

    fn complete_item(
        &self,
        parent: Option<usize>,
        outcome: ItemOutcome,
    ) {
        match outcome {
            ItemOutcome::FileOk => self.prog.file_ok(),
            // failed self-counting units (extraction aborting before any
            // entry was registered) must still block a CompleteOk finish
            ItemOutcome::Failed => self.prog.file_failed(),
            ItemOutcome::ExtractOk | ItemOutcome::Skipped => {}
        }
        if let Some(p) = parent {
            self.active().tracker.child_finished(p, self.dir_context());
        }
        if self.active().outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.try_finish();
        }
    }

    fn try_finish(&self) {
        let Some(job) = &self.job else { return };
        if !job.walk_done.load(Ordering::Acquire) {
            return;
        }
        if job.tracker.has_dirs() && !job.tracker.root_finished() {
            return;
        }
        let cancelled = job.token.is_cancelled();
        let snap = self.prog.snapshot();
        let failed = snap.files_failed > 0;
        let target = if cancelled {
            TaskState::Canceled
        } else if failed {
            TaskState::CompleteErr
        } else {
            if job.params.r#move && snap.files_failed == 0 {
                self.prog.cleanup_force_success();
            }
            TaskState::CompleteOk
        };

        let cur_state = self.prog.snapshot().state;
        let transitioned = if cur_state.is_terminal() {
            false
        } else {
            self.prog.cas_state(cur_state, target).is_ok()
        };

        if transitioned || cur_state == target {
            let s = self.prog.snapshot();
            self.log.info(format!(
                "finished {}: {} ok, {} failed{}, {} bytes",
                match target {
                    TaskState::CompleteOk => "ok",
                    TaskState::Canceled => "canceled",
                    TaskState::CompleteErr => "with errors",
                    _ => "finished",
                },
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

pub(crate) struct JobCtx {
    pub params: TransferParams,
    pub token: CancelToken,
    pub tracker: DirMetaTracker,
    outstanding: AtomicUsize,
    walk_done: AtomicBool,
}

struct Inner {
    shutting_down: AtomicBool,
    next_id: AtomicU64,
    tasks: Mutex<HashMap<TaskId, Arc<TaskEntry>>>,
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
            tasks: Mutex::new(HashMap::new()),
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
        match req.kind {
            JobKind::Transfer(params) => self.submit_transfer(req.source, req.dest, params),
            JobKind::Extract(_) => self.submit_extract(req.source, req.dest),
        }
    }

    fn complete_sync(
        &self,
        source: &std::path::Path,
        dest: &std::path::Path,
        is_move: bool,
        skipped: bool,
    ) -> TaskHandle {
        let prog = Arc::new(Progress::new(is_move));
        let log = Arc::new(TaskLog::default());
        log.info(format!(
            "{} {} -> {}",
            if is_move { "move" } else { "copy" },
            source.display(),
            dest.display()
        ));
        prog.register_file(0);
        if skipped {
            prog.skip_file();
        } else {
            prog.file_ok();
        }
        let _ = prog.cas_state(TaskState::Pending, TaskState::CompleteOk);
        self.register_task(prog, log, None).handle()
    }

    /// The worker-channel sender while submission is open. Shutdown
    /// revokes it by taking the sender out, so a `None` here is the one
    /// authority for refusing submissions — filesystem side effects must
    /// not be made on its behalf.
    fn work_tx(&self) -> Result<Sender<QueuedWork>, SubmitError> {
        // the guard covers only this lookup: callers receive a detached
        // clone and hold no lock while they work
        let sender = self.inner.tx.lock().expect("tx lock").as_ref().cloned();
        sender.ok_or(SubmitError::ShuttingDown)
    }

    /// Creates and registers a task entry under a freshly minted id,
    /// returning it. A `None` job registers a synchronous completion; an
    /// active job runs on workers. Registration pairs with the shutdown
    /// recheck: an arrival landing after cancel-all swept the map cancels
    /// the live job itself, so no task runs uncancelled past teardown.
    fn register_task(
        &self,
        prog: Arc<Progress>,
        log: Arc<TaskLog>,
        job: Option<JobCtx>,
    ) -> Arc<TaskEntry> {
        let entry = Arc::new(TaskEntry {
            id: self.inner.next_id.fetch_add(1, Ordering::Relaxed),
            prog,
            log,
            job,
        });
        self.inner
            .tasks
            .lock()
            .expect("tasks lock")
            .insert(entry.id, entry.clone());
        // checked after insertion on purpose: flag-set precedes cancel-all,
        // so a false here guarantees the sweep still sees this entry
        if self.inner.shutting_down.load(Ordering::Acquire) {
            entry.cancel();
        }
        entry
    }

    fn submit_extract(
        &self,
        source: PathBuf,
        dest: PathBuf,
    ) -> Result<TaskHandle, SubmitError> {
        let tx = self.work_tx()?;
        let prog = Arc::new(Progress::new(false));
        let log = Arc::new(TaskLog::default());
        log.info(format!(
            "extract {} -> {}",
            source.display(),
            dest.display()
        ));

        let job = JobCtx {
            params: TransferParams::default(),
            token: CancelToken::new(),
            tracker: DirMetaTracker::new(),
            outstanding: AtomicUsize::new(0),
            walk_done: AtomicBool::new(true),
        };
        let entry = self.register_task(prog, log, Some(job));

        let _ = entry.prog.cas_state(TaskState::Pending, TaskState::Started);
        entry
            .enqueue(&tx, None, WorkItem::Extract(ExtractJob { source, dest }))
            .map_err(|_| SubmitError::ShuttingDown)?;

        Ok(entry.handle())
    }

    fn submit_transfer(
        &self,
        source: PathBuf,
        dest: PathBuf,
        params: TransferParams,
    ) -> Result<TaskHandle, SubmitError> {
        if dest.starts_with(&source) && dest != source {
            return Err(SubmitError::IntoItself { source, dest });
        }

        let tx = self.work_tx()?;

        let is_move = params.r#move;
        let plan = match resolve_root(&source, &dest, is_move, params.root)? {
            RootOutcome::Proceed(plan) => {
                if self.inner.shutting_down.load(Ordering::Acquire) {
                    return Err(SubmitError::ShuttingDown);
                }
                plan
            }
            RootOutcome::DoneOk => {
                return Ok(self.complete_sync(&source, &dest, is_move, false));
            }
            RootOutcome::DoneSkipped => {
                return Ok(self.complete_sync(&source, &dest, is_move, true));
            }
        };

        let prog = Arc::new(Progress::new(is_move));
        let log = Arc::new(TaskLog::default());

        let job = JobCtx {
            params,
            token: CancelToken::new(),
            tracker: DirMetaTracker::new(),
            outstanding: AtomicUsize::new(1),
            walk_done: AtomicBool::new(false),
        };
        let entry = self.register_task(prog, log, Some(job));

        let h = spawn_collector(tx, entry.clone(), plan);
        self.inner
            .collectors
            .lock()
            .expect("collectors lock")
            .push(h);

        let _ = entry.prog.cas_state(TaskState::Pending, TaskState::Started);
        Ok(entry.handle())
    }

    pub fn cancel(
        &self,
        id: TaskId,
    ) {
        if let Some(task) = self.inner.tasks.lock().expect("tasks lock").get(&id) {
            task.cancel();
        }
    }

    pub fn cancel_all(&self) {
        for task in self.inner.tasks.lock().expect("tasks lock").values() {
            task.cancel();
        }
    }

    pub fn snapshot(&self) -> Vec<(TaskId, TaskSnapshot)> {
        self.inner
            .tasks
            .lock()
            .expect("tasks lock")
            .iter()
            .map(|(id, t)| (*id, t.prog.snapshot()))
            .collect()
    }

    pub fn fail_reason(
        &self,
        id: TaskId,
    ) -> Option<String> {
        self.inner
            .tasks
            .lock()
            .expect("tasks lock")
            .get(&id)
            .and_then(|t| t.log.first_error())
    }

    pub fn log_lines(
        &self,
        id: TaskId,
    ) -> Option<Vec<String>> {
        self.inner
            .tasks
            .lock()
            .expect("tasks lock")
            .get(&id)
            .map(|t| t.log.lines())
    }

    pub fn shutdown(
        &self,
        deadline: Duration,
    ) {
        self.inner.shutting_down.store(true, Ordering::Release);
        self.cancel_all();

        if let Some(tx) = self.inner.tx.lock().expect("tx lock").take() {
            drop(tx);
        }

        let collectors =
            std::mem::take(&mut *self.inner.collectors.lock().expect("collectors lock"));
        for h in collectors {
            let _ = h.join();
        }

        let deadline_instant = Instant::now() + deadline;
        while Instant::now() < deadline_instant {
            let all_done = self
                .inner
                .tasks
                .lock()
                .expect("tasks lock")
                .values()
                .all(|t| t.prog.snapshot().state.is_terminal());
            if all_done {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        let workers = std::mem::take(&mut *self.inner.workers.lock().expect("workers lock"));
        for h in workers {
            let _ = h.join();
        }
    }
}

fn finish_symlink_claim(
    source: &Path,
    dest: &Path,
    is_move: bool,
) -> Result<RootOutcome, SubmitError> {
    let mut final_dest = dest.to_path_buf();
    if let Some(p) = final_dest.parent() {
        if !p.as_os_str().is_empty() {
            fs::create_dir_all(p)
                .map_err(|e| SubmitError::Claim(p.to_path_buf(), ClaimError::Io(e)))?;
        }
    }
    let outcome = create_symlink_claim(source, &mut final_dest)?;
    if is_move {
        if let Err(e) = fs::remove_file(source) {
            let _ = fs::remove_file(&final_dest);
            return Err(SubmitError::Claim(source.to_path_buf(), ClaimError::Io(e)));
        }
    }
    Ok(outcome)
}

fn create_symlink_claim(
    source: &Path,
    dest: &mut PathBuf,
) -> Result<RootOutcome, SubmitError> {
    use super::copier::create_symlink;

    let err = |e: ClaimError| SubmitError::Claim(dest.clone(), e);
    let target = std::fs::read_link(source).map_err(|e| err(ClaimError::Io(e)))?;
    let desired = dest.clone();
    let parent = dest.parent().unwrap_or(Path::new("."));
    let stem_ext = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let [stem, ext] = cba::bath::split_ext(&stem_ext);

    if create_symlink(&target, &desired).is_ok() {
        return Ok(RootOutcome::DoneOk);
    }
    for i in 1..=9999u32 {
        let candidate = if ext.is_empty() {
            parent.join(format!("{stem}_{i}"))
        } else {
            parent.join(format!("{stem}_{i}.{ext}"))
        };
        if create_symlink(&target, &candidate).is_ok() {
            *dest = candidate;
            return Ok(RootOutcome::DoneOk);
        }
    }
    Err(err(ClaimError::Taken))
}

enum RootOutcome {
    Proceed(CollectorPlan),
    DoneOk,
    DoneSkipped,
}

fn resolve_identity(
    is_move: bool,
    src_is_dir: bool,
    root: RootStrategy,
    dest: &Path,
) -> Result<Option<RootOutcome>, SubmitError> {
    let err = |e: ClaimError| SubmitError::Claim(dest.to_path_buf(), e);
    if is_move {
        return Ok(Some(RootOutcome::DoneOk));
    }
    match root {
        RootStrategy::Rename => Ok(None),
        RootStrategy::Overwrite => Err(err(ClaimError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot overwrite itself",
        )))),
        RootStrategy::Fail => Err(err(ClaimError::Taken)),
        RootStrategy::Merge if !src_is_dir => Ok(Some(RootOutcome::DoneSkipped)),
        RootStrategy::Merge => Err(err(ClaimError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot merge a directory into itself",
        )))),
    }
}

fn resolve_symlink_root(
    source: &Path,
    dest: &Path,
    is_move: bool,
    root: RootStrategy,
) -> Result<RootOutcome, SubmitError> {
    let err = |e: ClaimError| SubmitError::Claim(dest.to_path_buf(), e);
    if fs::symlink_metadata(dest).is_ok() {
        match root {
            RootStrategy::Rename => {}
            RootStrategy::Overwrite => {
                let _ = fs::remove_file(dest);
            }
            RootStrategy::Merge => return Ok(RootOutcome::DoneSkipped),
            RootStrategy::Fail => {
                return Err(err(ClaimError::Taken));
            }
        }
    }
    finish_symlink_claim(source, dest, is_move)
}

fn resolve_root(
    source: &Path,
    dest: &Path,
    is_move: bool,
    root: RootStrategy,
) -> Result<RootOutcome, SubmitError> {
    let err = |e: ClaimError| SubmitError::Claim(dest.to_path_buf(), e);
    let meta = fs::symlink_metadata(source)
        .map_err(|_| SubmitError::SourceMissing(source.to_path_buf()))?;
    let src_is_dir = meta.is_dir();
    let src_is_symlink = meta.file_type().is_symlink();

    if dest == source {
        if let Some(outcome) = resolve_identity(is_move, src_is_dir, root, dest)? {
            return Ok(outcome);
        }
    }

    if src_is_symlink {
        return resolve_symlink_root(source, dest, is_move, root);
    }

    if let Ok(dst_md) = fs::symlink_metadata(dest) {
        let dst_is_dir = dst_md.is_dir();

        if src_is_dir && !dst_is_dir {
            if root == RootStrategy::Overwrite {
                fs::remove_file(dest).map_err(|e| err(ClaimError::Io(e)))?;
            } else if root != RootStrategy::Rename {
                return Err(err(ClaimError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "destination is a file",
                ))));
            }
        } else if !src_is_dir && dst_is_dir {
            if root != RootStrategy::Rename {
                // for safety due to our nonstandard treatment of resolving mv a -> b (b is directory) directly instead of conventional a -> b/a
                return Err(err(ClaimError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "destination is a directory",
                ))));
            }
        } else if !src_is_dir && root == RootStrategy::Merge {
            // A file target under RootStrategy::Merge skips without error
            return Ok(RootOutcome::DoneSkipped);
        }
    }

    claim_root(source, dest, src_is_dir, is_move, root)
}

fn claim_root(
    source: &Path,
    dest: &Path,
    src_is_dir: bool,
    is_move: bool,
    root: RootStrategy,
) -> Result<RootOutcome, SubmitError> {
    let err = |e: ClaimError| SubmitError::Claim(dest.to_path_buf(), e);

    let policy = root.claim_policy();

    if !src_is_dir {
        if is_move {
            match cba_claim::replace_file(source, dest, policy).map_err(err)? {
                None => Ok(RootOutcome::DoneOk),
                Some(claim) => Ok(RootOutcome::Proceed(CollectorPlan {
                    source: source.to_path_buf(),
                    claim: RootClaim::File(claim),
                    clear_dest: false,
                })),
            }
        } else {
            let claim = cba_claim::reserve_file_all(dest, policy).map_err(err)?;
            Ok(RootOutcome::Proceed(CollectorPlan {
                source: source.to_path_buf(),
                claim: RootClaim::File(claim),
                clear_dest: false,
            }))
        }
    } else {
        if is_move && (root != RootStrategy::Merge) {
            match cba_claim::replace_dir(source, dest, policy).map_err(err)? {
                None => Ok(RootOutcome::DoneOk),
                Some(claim) => {
                    let clear_dest = root == RootStrategy::Overwrite;
                    Ok(RootOutcome::Proceed(CollectorPlan {
                        source: source.to_path_buf(),
                        claim: RootClaim::Dir(claim),
                        clear_dest,
                    }))
                }
            }
        } else {
            let claim = cba_claim::reserve_dir_all(dest, policy).map_err(err)?;
            let clear_dest = root == RootStrategy::Overwrite;
            Ok(RootOutcome::Proceed(CollectorPlan {
                source: source.to_path_buf(),
                claim: RootClaim::Dir(claim),
                clear_dest,
            }))
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
                let task = {
                    inner
                        .tasks
                        .lock()
                        .expect("tasks lock")
                        .get(&w.task)
                        .cloned()
                };
                let Some(task) = task else { continue };
                let Some(job) = &task.job else { continue };
                if job.token.is_cancelled() {
                    task.complete_item(w.parent, ItemOutcome::Skipped);
                    continue;
                }
                if scratch.len() < job.params.buffer_size.get() {
                    scratch.resize(job.params.buffer_size.get(), 0);
                }
                let parent = w.parent;
                let outcome = copier::execute(&task, w, &mut scratch[..]);
                task.complete_item(parent, outcome);
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
    tx: Sender<QueuedWork>,
    task: Arc<TaskEntry>,
    plan: CollectorPlan,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("fist-copy-walk".to_string())
        .spawn(move || collector_run(task, plan, tx))
        .expect("spawn copy walker")
}

fn collector_run(
    task: Arc<TaskEntry>,
    plan: CollectorPlan,
    tx: Sender<QueuedWork>,
) {
    let cancelled = task.active().token.is_cancelled();
    if cancelled {
        let _ = task.prog.cas_state(TaskState::Pending, TaskState::Canceled);
    } else {
        let _ = task.prog.cas_state(TaskState::Pending, TaskState::Started);
        let src_display = plan.source.display().to_string();
        let collect_result = walker::collect(&task, plan, &tx);
        match collect_result {
            Ok(()) => {}
            Err(WalkAbort::Canceled) => {
                let _ = task.prog.cas_state(TaskState::Started, TaskState::Canceled);
            }
            Err(WalkAbort::Io(e)) => {
                task.log.error(format!("walk of {src_display} failed: {e}"));
                task.prog.file_failed();
                let _ = task
                    .prog
                    .cas_state(TaskState::Started, TaskState::CompleteErr);
            }
        }
    }
    let job = task.active();
    job.walk_done.store(true, Ordering::Release);
    if job.outstanding.fetch_sub(1, Ordering::AcqRel) == 1 {
        task.try_finish();
    }
}
