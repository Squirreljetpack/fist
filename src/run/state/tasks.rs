#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]
//! Background task and child-process management.

use std::{cell::RefCell, collections::BTreeMap, ops::RangeBounds, sync::Mutex};

use cba::{_wbog, dbog, ebog, ibog, wbog};
use tokio::{self, task::JoinSet};

thread_local! {
    static JOINSET: RefCell<JoinSet<()>> = RefCell::new(JoinSet::new());
}

/// Descriptions of in-flight tasks, keyed by name and counted for duplicates,
/// so shutdown can name what it is waiting on. Process-global because the
/// multi-thread runtime polls tasks on arbitrary worker threads, and entries
/// are written both at spawn and at task completion.
static TASK_NAMES: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());

/// Removes this task's description from [`TASK_NAMES`] when the task ends,
/// even if it panics.
struct TaskNameGuard(String);

impl Drop for TaskNameGuard {
    fn drop(&mut self) {
        let mut names = TASK_NAMES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match names.get_mut(&self.0) {
            Some(count) if *count > 1 => *count -= 1,
            _ => {
                names.remove(&self.0);
            }
        }
    }
}

/// Records `desc` as an in-flight task description.
fn register_name(desc: String) {
    let mut names = TASK_NAMES.lock().unwrap();
    *names.entry(desc).or_insert(0) += 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskId {
    Populate = 0,
    Lessfilter = 3,
    Batch = 32,
}

static JOBS: Mutex<BTreeMap<u8, Vec<std::process::Child>>> = Mutex::new(BTreeMap::new());
static ZOMBIES: Mutex<Vec<std::process::Child>> = Mutex::new(Vec::new());

/// Namespace for spawning/tracking background futures and child processes.
pub struct TASKS;

impl TASKS {
    /// Register a child process for tracking.
    /// If id < 32, any existing processes for this ID will be killed and moved to ZOMBIES.
    /// If id >= 32, the child is appended to the existing list for that ID.
    pub fn register_child(id: TaskId, child: std::process::Child) {
        let mut jobs = JOBS.lock().unwrap();
        let id_u8 = id as u8;

        if id_u8 < 32 {
            if let Some(old_vec) = jobs.insert(id_u8, vec![child]) {
                let mut zombies = ZOMBIES.lock().unwrap();
                for mut old in old_vec {
                    let _ = old.kill();
                    zombies.push(old);
                }
            }
        } else {
            jobs.entry(id_u8).or_default().push(child);
        }
    }

    /// Kill all processes associated with a specific TaskId.
    pub fn kill_child(id: TaskId) {
        let mut jobs = JOBS.lock().unwrap();
        if let Some(old_vec) = jobs.remove(&(id as u8)) {
            let mut zombies = ZOMBIES.lock().unwrap();
            for mut old in old_vec {
                let _ = old.kill();
                zombies.push(old);
            }
        }
    }

    /// Kill all processes whose IDs fall within the given range.
    pub fn kill_children(range: impl RangeBounds<u8>) {
        let mut jobs = JOBS.lock().unwrap();
        let mut zombies = ZOMBIES.lock().unwrap();

        let keys: Vec<u8> = jobs
            .keys()
            .filter(|&&id| range.contains(&id))
            .copied()
            .collect();
        for k in keys {
            if let Some(old_vec) = jobs.remove(&k) {
                for mut old in old_vec {
                    let _ = old.kill();
                    zombies.push(old);
                }
            }
        }
    }

    /// Clean up exited processes from JOBS and ZOMBIES.
    /// Returns the number of processes still in the ZOMBIES list.
    pub fn prune_children() -> usize {
        if let Ok(mut jobs) = JOBS.lock() {
            for v in jobs.values_mut() {
                v.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
            }
            jobs.retain(|_, v| !v.is_empty());
        }
        let mut zombies = ZOMBIES.lock().unwrap();
        zombies.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
        zombies.len()
    }

    /// Spawn a background future tracked under the given description. The
    /// description is shown by [`TASKS::shutdown`] while it waits.
    pub fn spawn<F>(desc: impl Into<String>, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let desc = desc.into();
        register_name(desc.clone());
        JOINSET.with(|tasks| {
            tasks.borrow_mut().spawn(async move {
                let _guard = TaskNameGuard(desc);
                fut.await;
            });
        });
    }

    /// Spawn a blocking background task tracked under the given description.
    /// The description is shown by [`TASKS::shutdown`] while it waits.
    pub fn spawn_blocking<F>(desc: impl Into<String>, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let desc = desc.into();
        register_name(desc.clone());
        JOINSET.with(|tasks| {
            tasks.borrow_mut().spawn_blocking(move || {
                let _guard = TaskNameGuard(desc);
                f();
            });
        });
    }

    pub async fn shutdown(initial_warn_ms: u64, warn_secs: u64, max_secs: u64) {
        use tokio::time::{self, Duration};

        Self::kill_children(..);

        let mut join_set = JOINSET.with(|tasks| std::mem::take(&mut *tasks.borrow_mut()));

        if join_set.is_empty() {
            if Self::prune_children() > 0 {
                _wbog!("Some background processes are still terminating...");
            }
            return;
        }

        let mut warned = false;

        dbog!("Waiting on {} tasks.", join_set.len());

        let warn_deadline = time::sleep(Duration::from_millis(initial_warn_ms));
        tokio::pin!(warn_deadline);

        let max_deadline = time::sleep(Duration::from_secs(max_secs));
        tokio::pin!(max_deadline);

        loop {
            tokio::select! {
                // task completed
                res = join_set.join_next() => {
                    match res {
                        Some(_) => {
                            if join_set.is_empty() {
                                if warned {
                                    ibog!(
                                        "All tasks finished"
                                    );
                                };
                                break
                            } else {
                                _wbog!(
                                    "Waiting on {} task(s).",
                                    join_set.len()
                                );
                            };
                        }
                        None => {
                            if warned {
                                ibog!(
                                    "All tasks finished"
                                );
                            }
                            break
                        },
                    }
                }

                _ = &mut warn_deadline => {
                    if !join_set.is_empty() {
                        wbog!(
                            "{} (Press Ctrl-C to exit).",
                            waiting_on(join_set.len())
                        );
                        warned = true;

                        warn_deadline
                        .as_mut()
                        .reset(time::Instant::now() + Duration::from_secs(warn_secs));
                    }
                }

                _ = &mut max_deadline => {
                    ebog!(
                        "Shutdown timeout reached. Aborting {} task(s).",
                        join_set.len()
                    );

                    join_set.shutdown().await;
                    break;
                }
            }
        }

        if Self::prune_children() > 0 {
            _wbog!("Some background processes are still terminating...");
        }
    }
}

/// Formats the shutdown wait message: names the first outstanding task's
/// description and counts the rest, e.g. `Waiting on db stash and 2 others.`
fn waiting_on(total: usize) -> String {
    let names = TASK_NAMES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match names.keys().next() {
        Some(name) if total <= 1 => format!("Waiting on {name}."),
        Some(name) => {
            let others = total - 1;
            let plural = if others == 1 { "" } else { "s" };
            format!("Waiting on {name} and {others} other{plural}.")
        }
        None => format!("Waiting on {total} task(s)."),
    }
}
