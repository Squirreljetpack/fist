#![allow(non_snake_case)]
use std::{cell::OnceCell, sync::OnceLock};

use cba::bait::ResultExt;
use log::debug;
use matchmaker::{action::Action, event::RenderSender};

use crate::config::GlobalConfig;
use crate::{
    db::{Connection, Pool, zoxide::HistoryConfig},
    errors::DbError,
    run::{FsPane, action::FsAction},
    spawn::menu_action::MenuActions,
    watcher::{WatcherMessage, WatcherSender},
};
use fist_types::filters::SortOrder;

mod filters;
pub use filters::*;
pub mod sort;
mod stack;
pub use stack::*;
pub mod context;
mod temp;
pub mod toast;
pub use toast::{TOAST, ToastContent, ToastStyle};
pub mod ui;
pub use temp::*;

// ------------- TRACKING -----------------------

pub static DB_FILTER: OnceLock<HistoryConfig> = OnceLock::new();
/// The user's custom menu actions, registered at startup so the execute
/// handlers can look up an action by key (discriminant 7).
pub static MENU_ACTIONS: OnceLock<MenuActions> = OnceLock::new();
// ------------- READ_ONLY ------------------------
pub mod GLOBAL {
    use matchmaker::{event::BindSender, message::BindDirective};

    use super::*;
    thread_local! {
        static CONFIG: OnceCell<GlobalConfig> = const { OnceCell::new() };
        static DB: OnceCell<Pool> = const { OnceCell::new() };
        static BIND_TX: OnceCell<BindSender<FsAction>> = const { OnceCell::new() };
    }
    static RENDER_TX: OnceLock<RenderSender<FsAction>> = OnceLock::new();
    static WATCHER_TX: OnceLock<WatcherSender> = OnceLock::new();

    /// All global methods can be called iff this has been called
    /// DB_FILTER needs to be initialized separately
    pub fn init(
        cfg: GlobalConfig,
        render_tx: RenderSender<FsAction>,
        watcher_tx: WatcherSender,
        db_pool: Pool,
        pane: FsPane,
        bind_tx: BindSender<FsAction>,
    ) {
        // need to handle the patterns listened on by sync_handler
        let sort = pane.sort_order();
        let visibility = match &pane {
            FsPane::Nav { vis, .. }
            | FsPane::Custom { vis, .. }
            | FsPane::Find { vis, .. }
            | FsPane::Search { vis, .. } => *vis,
            _ => Default::default(),
        };
        debug!("Initial filters: {sort}, {visibility:?}");
        FILTERS::set(visibility);

        CONFIG.with(|c| c.set(cfg).expect("GLOBAL::init called twice"));
        RENDER_TX.set(render_tx).expect("GLOBAL::init called twice");
        WATCHER_TX
            .set(watcher_tx)
            .expect("GLOBAL::init called twice");
        DB.with(|d| d.set(db_pool).expect("GLOBAL::init called twice"));
        BIND_TX.with(|d| d.set(bind_tx).expect("GLOBAL::init called twice"));
        STACK::init(pane);
    }

    /// must be called in initializing thread
    pub fn with_cfg<F, R>(f: F) -> R
    where
        F: FnOnce(&GlobalConfig) -> R,
    {
        CONFIG.with(|c| f(c.get().expect("GLOBAL::init not called")))
    }

    // ------------ SENDERS --------------
    pub fn send_action(action: impl Into<Action<FsAction>>) {
        RENDER_TX
            .get()
            .expect("render tx missing")
            .send(matchmaker::message::RenderCommand::Action(action.into()))
            ._elog();
    }

    pub fn send_mm(msg: matchmaker::message::RenderCommand<FsAction>) {
        RENDER_TX
            .get()
            .expect("render tx missing")
            .send(msg)
            ._elog();
    }

    pub fn send_watcher(msg: WatcherMessage) {
        WATCHER_TX
            .get()
            .expect("watcher tx missing")
            .send(msg)
            ._elog();
    }
    pub fn send_bind(msg: BindDirective<FsAction>) {
        BIND_TX.with(|tx| {
            tx.get().expect("bind tx missing").send(msg)._elog();
        });
    }

    // ------------ DB ---------------------------
    /// must be called in initializing thread
    pub fn db() -> Pool {
        DB.with(|cell| cell.get().expect("GLOBAL::init not called").clone())
    }

    pub async fn get_db_entries(
        conn: &mut Connection,
        sort: SortOrder,
    ) -> Result<Vec<crate::db::Entry>, DbError> {
        let config = DB_FILTER.get().expect("DB_FILTER not initialized");
        conn.get_entries(sort, config, conn.table).await
    }
}

// -----------------------------------------

pub mod APP {
    use std::sync::atomic::AtomicBool;

    /// ensure recache isn't run more than once
    pub static RAN_RECACHE: AtomicBool = const { AtomicBool::new(false) };
}

// -------------------------------------------
pub mod TASKS {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::ops::RangeBounds;
    use std::sync::Mutex;

    use cba::{_wbog, dbog, ebog, ibog, wbog};
    use tokio::{self, task::JoinSet};

    thread_local! {
        static TASKS: RefCell<JoinSet<()>> = RefCell::new(JoinSet::new());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TaskId {
        Populate = 0,
        Lessfilter = 3,
        Batch = 32,
    }

    static JOBS: Mutex<BTreeMap<u8, Vec<std::process::Child>>> = Mutex::new(BTreeMap::new());
    static ZOMBIES: Mutex<Vec<std::process::Child>> = Mutex::new(Vec::new());

    /// Register a child process for tracking.
    /// If id < 32, any existing processes for this ID will be killed and moved to ZOMBIES.
    /// If id >= 32, the child is appended to the existing list for that ID.
    pub fn register_child(
        id: TaskId,
        child: std::process::Child,
    ) {
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

    pub fn spawn<F>(fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        TASKS.with(|tasks| {
            tasks.borrow_mut().spawn(fut);
        });
    }

    pub fn spawn_blocking<F>(f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        TASKS.with(|tasks| {
            tasks.borrow_mut().spawn_blocking(f);
        });
    }

    pub async fn shutdown(
        initial_warn_ms: u64,
        warn_secs: u64,
        max_secs: u64,
    ) {
        use tokio::time::{self, Duration};

        kill_children(..);

        let mut join_set = TASKS.with(|tasks| std::mem::take(&mut *tasks.borrow_mut()));

        if join_set.is_empty() {
            if prune_children() > 0 {
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
                            "Waiting on {} task(s). (Press Ctrl-C to exit).",
                            join_set.len()
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

        if prune_children() > 0 {
            _wbog!("Some background processes are still terminating...");
        }
    }
}
