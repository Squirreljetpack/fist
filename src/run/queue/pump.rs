//! Bridge between the [`fist_copy`] engine and queue rows.
//!
//! Owns the process-global scheduler and ONE watcher thread that mirrors
//! engine state into queue rows. The watcher is scheduler-driven:
//! `scheduler().snapshot()` is the source of truth; new task ids are
//! resolved to their rows exactly once (the status clone is cached for the
//! task's lifetime), progress/size atomics are updated per tick, and
//! terminal snapshots CAS-finalize the row — the CAS is the once-only
//! "seen" flag, so toasts and history fire exactly once per task no matter
//! how many ticks observe them.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;

use fist_copy::{Scheduler, TaskState};

use super::*;
use crate::run::{
    item::short_display,
    state::{GLOBAL::db, TOAST, ToastFlags, ToastStyle},
};

pub static COPY_SCHED: OnceLock<Scheduler> = OnceLock::new();

#[derive(Debug, Default, Clone, Copy)]
pub struct PumpConfig {
    pub bump_dests: bool,
}

pub static PUMP_CONFIG: OnceLock<PumpConfig> = OnceLock::new();

pub fn maybe_bump_dest(item: &QueueItem) {
    let bump = PUMP_CONFIG.get().is_some_and(|c| c.bump_dests);
    if !item.data.is_empty() && DEST_KINDS.contains(&item.kind.as_str()) && bump {
        let dest = AbsPath::new_unchecked(PathBuf::from(&item.data));
        db().bump_path(dest.is_dir(), dest);
    }
}

static WATCHER: OnceLock<()> = OnceLock::new();

const TICK: Duration = Duration::from_millis(100);

/// The process-global transfer scheduler, created on first use.
pub fn scheduler() -> &'static Scheduler {
    COPY_SCHED.get_or_init(|| Scheduler::new(Default::default()))
}

/// Starts the single scheduler-driven watcher on the first engine-backed
/// submission. Lazy, so nothing can forget to initialize it.
pub fn ensure_watcher() {
    WATCHER.get_or_init(|| {
        thread::Builder::new()
            .name("fist-copy-watch".to_string())
            .spawn(|| {
                let mut seen = HashMap::new();
                let mut finalized = HashSet::new();
                loop {
                    thread::sleep(TICK);
                    tick(&mut seen, &mut finalized);
                }
            })
            .expect("spawn copy watcher");
    });
}

/// Log lines recorded so far for an in-flight task (engine-side TaskLog).
pub fn task_log(task_id: u64) -> Option<Vec<String>> {
    scheduler().log_lines(task_id)
}

/// Shut down the global scheduler if it was ever created.
pub fn shutdown(drain_timeout: Duration) {
    if let Some(sched) = COPY_SCHED.get() {
        sched.shutdown(drain_timeout);
    }
}

/// One tick: discover rows for unseen task ids, then mirror every snapshot.
fn tick(
    seen: &mut HashMap<u64, QueueItem>,
    finalized: &mut HashSet<u64>,
) {
    // finished ids stay in `finalized` forever: snapshots are never
    // evicted from the engine, so dropping them from the watch set would
    // make every later tick rediscover them and rescan the queue
    let snaps: Vec<_> = scheduler()
        .snapshot()
        .into_iter()
        .filter(|(id, _)| !finalized.contains(id))
        .collect();
    discover(&snaps, seen);
    for (id, snap) in snaps {
        let Some(item) = seen.get_mut(&id) else {
            continue;
        };
        mirror(&item.status, &snap);
        match snap.state {
            TaskState::Pending | TaskState::Started => {}
            TaskState::CompleteOk => finalize(item, QueueItemState::CompleteOk, |item| {
                maybe_bump_dest(item);
                if let Some(first) = item.src.first() {
                    TOAST::push(ToastStyle::Success, "Complete: ", [short_display(first)]);
                }
            }),
            TaskState::CompleteErr | TaskState::Canceled => {
                let why = scheduler().fail_reason(id).unwrap_or_else(|| {
                    if snap.state == TaskState::Canceled {
                        "canceled".to_string()
                    } else if snap.files_failed > 0 {
                        format!("{} file(s) failed", snap.files_failed)
                    } else {
                        "task failed".to_string()
                    }
                });
                finalize(item, QueueItemState::CompleteErr, |item| {
                    if let Some(first) = item.src.first() {
                        TOAST::push(ToastStyle::Error, "Failed: ", [short_display(first)]);
                    }
                    TOAST::notice(ToastStyle::Error, why);
                });
            }
        }
        // terminal rows leave the watch set; their snapshots stay readable
        // in the engine for [`task_log`]
        if item.status.state.is_complete() {
            seen.remove(&id);
            finalized.insert(id);
        }
    }
}

/// Resolves rows for task ids not watched yet. Statuses are cloned out
/// from under the lock once; from then on the watcher talks to the shared
/// atomics directly, so removing a running row cannot lose its completion.
fn discover(
    snaps: &[(u64, fist_copy::TaskSnapshot)],
    seen: &mut HashMap<u64, QueueItem>,
) {
    let fresh: Vec<u64> = snaps
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| !seen.contains_key(id))
        .collect();
    if fresh.is_empty() {
        return;
    }
    let state = QUEUE_STATE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for id in fresh {
        if let Some(item) = state
            .shared
            .iter()
            .find(|i| i.status.task_id() == Some(id) && i.status.state.is_started())
        {
            // extraction rows announce themselves here: discovery happens
            // exactly once per task, so the persistent "Extracting" toast
            // is pushed exactly once and retired by [`finalize`]
            if item.kind == "extract"
                && let Some(first) = item.src.first()
            {
                TOAST::push_with_flag(
                    ToastStyle::Info,
                    "Extracting: ",
                    [short_display(first)],
                    ToastFlags::PERSIST,
                );
            }
            seen.insert(id, item.clone());
        }
    }
}

fn mirror(
    status: &QueueItemStatus,
    snap: &fist_copy::TaskSnapshot,
) {
    status
        .progress
        .store(percent_u8(snap), std::sync::atomic::Ordering::Relaxed);
    if snap.total_bytes > 0 {
        status
            .size
            .store(snap.total_bytes, std::sync::atomic::Ordering::Relaxed);
    }
}

/// CAS-gated row finalization: the winner of the transition runs `announce`
/// exactly once; later ticks lose the CAS and do nothing.
fn finalize(
    item: &mut QueueItem,
    target: QueueItemState,
    announce: impl FnOnce(&QueueItem),
) {
    if item
        .status
        .state
        .compare_exchange(QueueItemState::Started, target)
        .is_err()
    {
        return;
    }
    announce(item);
    QUEUE_ACTION_HISTORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(item.clone());
    if item.kind == "extract" {
        register_extraction(item, target);
        if target == QueueItemState::CompleteErr {
            TOAST::notice(ToastStyle::Error, "Extraction failed");
        }
        // retire the persistent toast pushed when entering the archive
        if let Some(first) = item.src.first() {
            TOAST::pop("Extracting: ", &short_display(first));
        }
    }
}

/// Record the finished extraction in the mount registry: that is what makes it
/// enterable (and visible in the mounts picker) once its contents are settled.
/// `data` carries the destination directory. The per-process extraction root
/// is removed as a whole on exit, so no row claims cleanup.
fn register_extraction(
    item: &QueueItem,
    target: QueueItemState,
) {
    let Some(source) = item.src.first() else {
        return;
    };
    let status = if target == QueueItemState::CompleteOk {
        crate::mount::MountStatus::Success
    } else {
        crate::mount::MountStatus::Failed
    };
    crate::mount::MountRegistry::register(crate::mount::MountRow::extraction(
        source.inner(),
        PathBuf::from(item.data.as_os_str()),
        status,
    ));
}

fn percent_u8(snap: &fist_copy::TaskSnapshot) -> u8 {
    let p = snap.percent().clamp(0.0, 100.0);
    (p * 255.0 / 100.0).round() as u8
}
