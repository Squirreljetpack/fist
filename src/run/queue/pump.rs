//! Bridge between the [`fist_copy`] engine and queue rows.
//!
//! Owns the process-global scheduler and a registry of in-flight tasks. Each
//! submitted task gets a small watcher thread that maps engine snapshots onto
//! the row's atomic [`QueueItemStatus`] and finalizes the row (toasts, action
//! history) when the task reaches a terminal state.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use fist_copy::{Scheduler, TaskHandle, TaskState};

use super::*;
use crate::run::{
    item::short_display,
    state::{TOAST, ToastStyle},
};

pub static COPY_SCHED: OnceLock<Scheduler> = OnceLock::new();

static COPY_TASKS: LazyLock<Mutex<HashMap<u64, TaskHandle>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The process-global transfer scheduler, created on first use.
pub fn scheduler() -> &'static Scheduler {
    COPY_SCHED.get_or_init(|| Scheduler::new(Default::default()))
}

/// Track `handle` against a queue row: a watcher thread refreshes its atomics
/// until the task reaches a terminal state, then finalizes the row.
pub fn register_task(
    handle: TaskHandle,
    item: &QueueItem,
) {
    let status = item.status.clone();
    let tracked = item.clone();
    COPY_TASKS.lock().unwrap().insert(handle.id, handle.clone());
    thread::Builder::new()
        .name("fist-copy-watch".to_string())
        .spawn(move || watch(handle, status, tracked))
        .expect("spawn copy watcher");
}

fn watch(
    handle: TaskHandle,
    status: QueueItemStatus,
    item: QueueItem,
) {
    loop {
        thread::sleep(Duration::from_millis(100));
        let snap = handle.snapshot();

        status
            .progress
            .store(percent_u8(&snap), std::sync::atomic::Ordering::Relaxed);
        if snap.total_bytes > 0 {
            status
                .size
                .store(snap.total_bytes, std::sync::atomic::Ordering::Relaxed);
        }

        match snap.state {
            TaskState::Pending | TaskState::Started => {}
            TaskState::CompleteOk => {
                if status
                    .state
                    .compare_exchange(QueueItemState::Started, QueueItemState::CompleteOk)
                    .is_ok()
                {
                    if let Some(first) = item.src.first() {
                        TOAST::push(ToastStyle::Success, "Complete: ", [short_display(first)]);
                    }
                    QUEUE_ACTION_HISTORY.lock().unwrap().push(item.clone());
                }
                break;
            }
            TaskState::CompleteErr | TaskState::Canceled => {
                if status
                    .state
                    .compare_exchange(QueueItemState::Started, QueueItemState::CompleteErr)
                    .is_ok()
                {
                    let why = if snap.state == TaskState::Canceled {
                        "canceled".to_string()
                    } else {
                        format!("{} file(s) failed", snap.files_failed)
                    };
                    if let Some(first) = item.src.first() {
                        TOAST::push(ToastStyle::Error, "Failed: ", [short_display(first)]);
                    }
                    TOAST::notice(ToastStyle::Error, why);
                }
                break;
            }
        }
    }
    COPY_TASKS.lock().unwrap().remove(&handle.id);
}

/// Log lines recorded so far for an in-flight task.
pub fn task_log(task_id: u64) -> Option<Vec<String>> {
    let map = COPY_TASKS.lock().unwrap();
    map.get(&task_id).map(|h| h.log_lines())
}

/// Shut down the global scheduler if it was ever created.
pub fn shutdown(drain_timeout: Duration) {
    if let Some(sched) = COPY_SCHED.get() {
        sched.shutdown(drain_timeout);
    }
}

fn percent_u8(snap: &fist_copy::TaskSnapshot) -> u8 {
    let p = snap.percent().clamp(0.0, 100.0);
    (p * 255.0 / 100.0).round() as u8
}
