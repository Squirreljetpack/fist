#![allow(non_snake_case)]
use std::{cell::RefCell, sync::Mutex};

use cba::bait::ResultExt;
use log::debug;
use matchmaker::{action::Action, event::RenderSender};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use tokio;

use crate::config::GlobalConfig;
use crate::{
    db::{Connection, DbSortOrder, Pool, zoxide::DbFilter},
    errors::DbError,
    run::{FsPane, action::FsAction},
    utils::text::{ToastContent, ToastStyle, make_toast},
    watcher::{WatcherMessage, WatcherSender},
};

mod filters;
pub use filters::*;
mod stack;
pub use stack::*;
pub mod context;
mod temp;
pub mod ui;
pub use temp::*;

// ------------- TRACKING -----------------------

// just try different kinds of locks :p
pub static DB_FILTER: tokio::sync::Mutex<Option<DbFilter>> =
    const { tokio::sync::Mutex::const_new(None) };
// ------------- READ_ONLY ------------------------
pub mod GLOBAL {
    use matchmaker::{event::BindSender, message::BindDirective};

    use crate::config::StashLogicConfig;

    use super::*;
    thread_local! {
        static CONFIG: RefCell<Option<GlobalConfig>> = const { RefCell::new(None) };
        static DB: RefCell<Option<Pool>> = const { RefCell::new(None) };
        static BIND_TX: RefCell<Option<BindSender<FsAction>>> = const { RefCell::new(None) };
    }
    static RENDER_TX: Mutex<Option<RenderSender<FsAction>>> = const { Mutex::new(None) };
    static WATCHER_TX: Mutex<Option<WatcherSender>> = const { Mutex::new(None) };

    /// All global methods can be called iff this has been called
    /// DB_FILTER needs to be initialized seperately with async
    pub fn init(
        cfg: GlobalConfig,
        stash_cfg: StashLogicConfig,
        render_tx: RenderSender<FsAction>,
        watcher_tx: WatcherSender,
        db_pool: Pool,
        pane: FsPane,
        bind_tx: BindSender<FsAction>,
    ) {
        // need to handle the patterns listened on by sync_handler
        let sort = match &pane {
            FsPane::Nav { sort, .. }
            | FsPane::Custom { sort, .. }
            | FsPane::Find { sort, .. }
            | FsPane::Stream { sort, .. } => *sort,
            FsPane::Folders { sort, .. } | FsPane::Files { sort, .. } => (*sort).into(),
            _ => Default::default(),
        };
        let visibility = match &pane {
            FsPane::Nav { vis, .. }
            | FsPane::Custom { vis, .. }
            | FsPane::Find { vis, .. }
            | FsPane::Stream { vis, .. }
            | FsPane::Search { vis, .. } => *vis,
            _ => Default::default(),
        };
        debug!("Initial filters: {sort}, {visibility:?}");
        FILTERS::set(sort, visibility);

        crate::run::stash::STASH::init(stash_cfg.modes.clone());

        CONFIG.with(|c| *c.borrow_mut() = Some(cfg));
        *RENDER_TX.lock().unwrap() = Some(render_tx);
        *WATCHER_TX.lock().unwrap() = Some(watcher_tx);
        DB.with(|d| *d.borrow_mut() = Some(db_pool));
        BIND_TX.with(|d| *d.borrow_mut() = Some(bind_tx));
        STACK::init(pane);
    }

    /// must be called in initializing thread
    pub fn with_cfg<F, R>(f: F) -> R
    where
        F: FnOnce(&GlobalConfig) -> R,
    {
        CONFIG.with(|c| f(c.borrow().as_ref().unwrap()))
    }

    // ------------ SENDERS --------------
    pub fn send_action(action: impl Into<Action<FsAction>>) {
        let guard = RENDER_TX.lock().unwrap();
        let tx = guard.as_ref().expect("render tx missing");

        tx.send(matchmaker::message::RenderCommand::Action(action.into()))
            ._elog();
    }

    pub fn send_mm(msg: matchmaker::message::RenderCommand<FsAction>) {
        let guard = RENDER_TX.lock().unwrap();
        let tx = guard.as_ref().expect("render tx missing");

        tx.send(msg)._elog();
    }

    /// must be called in initializing thread
    pub fn send_watcher(msg: WatcherMessage) {
        let guard = WATCHER_TX.lock().unwrap();
        let tx = guard.as_ref().expect("render tx missing");
        tx.send(msg)._elog();
    }
    pub fn send_bind(msg: BindDirective<FsAction>) {
        BIND_TX.with(|tx| {
            let guard = tx.borrow();
            let tx = guard.as_ref().expect("watcher tx missing");
            tx.send(msg)._elog();
        });
    }

    // ------------ DB ---------------------------
    /// must be called in initializing thread
    pub fn db() -> Pool {
        DB.with(|cell| cell.borrow().as_ref().unwrap().clone())
    }

    pub async fn get_db_entries(
        conn: &mut Connection,
        sort: DbSortOrder,
    ) -> Result<Vec<crate::db::Entry>, DbError> {
        let mut guard = DB_FILTER.lock().await;
        let db_filter = guard.as_mut().unwrap();
        if conn.table_name != "dirs" {
            let o = std::mem::take(&mut db_filter.resolve_symlinks);
            let ret = conn.get_entries(sort, db_filter).await;
            db_filter.resolve_symlinks = o;
            ret
        } else {
            conn.get_entries(sort, db_filter).await
        }
    }
}

// ------------- TOAST ----------------------------
static TOAST: Mutex<Vec<(Span<'static>, ToastContent)>> = Mutex::new(Vec::new());

pub struct TOAST {}

impl TOAST {
    pub fn clear() {
        let mut state = TOAST.lock().unwrap();
        state.clear();
        debug!("Cleared toasts: {state:?}");
        GLOBAL::send_action(FsAction::set_footer(None));
    }

    // todo: maintain a counter
    pub fn push_skipped() {
        let mut state = TOAST.lock().unwrap();

        const SKIPPED: &str = "Skipped";

        if let Some((_, ToastContent::Line(existing))) = state.iter_mut().find(|(span, content)| {
            span.content.is_empty()
                && matches!(
                    content,
                    ToastContent::Line(l)
                    if l.spans.first().map(|s| s.content.starts_with(SKIPPED)) == Some(true)
                )
        }) {
            let first = &existing.spans[0].content;

            let next = if first == SKIPPED {
                2
            } else {
                first
                    .strip_prefix(SKIPPED)
                    .and_then(|rest| {
                        rest.trim_start_matches('(')
                            .trim_end_matches(')')
                            .parse::<usize>()
                            .ok()
                    })
                    .map(|n| n + 1)
                    .unwrap_or(2)
            };

            existing.spans[0] =
                Span::styled(format!("{SKIPPED} ({next})"), Style::new().dim().italic());
        } else {
            let prefix_span = Span::raw("");
            let line = Line::from(Span::styled(SKIPPED, Style::new().dim().italic()));
            state.push((prefix_span, ToastContent::Line(line)));
        }

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    pub fn clear_msgs() {
        let mut state = TOAST.lock().unwrap();

        // Keep only entries whose span is not empty
        state.retain(|(span, _)| !span.content.is_empty());

        GLOBAL::send_action(FsAction::set_footer(None));
    }

    /// Push an item to a prefix group
    pub fn push(
        style: ToastStyle,
        prefix: &'static str,
        items: impl IntoIterator<Item = Span<'static>>,
    ) {
        let mut state = TOAST.lock().unwrap();
        if let Some((_, existing_content)) =
            state.iter_mut().find(|(p, _)| p.content.as_ref() == prefix)
        {
            if let ToastContent::List(existing_items) = existing_content {
                for i in items {
                    if !existing_items.contains(&i) {
                        existing_items.push(i);
                    }
                }
            } else {
                // Overwrite if not already a list
                *existing_content = ToastContent::List(items.into_iter().collect());
            }
        } else {
            let prefix_span = Span::styled(prefix, style);
            state.push((prefix_span, ToastContent::List(items.into_iter().collect())));
        }

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a pair of items a -> b, described by a prefix
    pub fn pair(
        style: ToastStyle,
        prefix: &'static str,
        from: Span<'static>,
        to: Span<'static>,
    ) {
        let mut state = TOAST.lock().unwrap();
        let prefix_span = Span::styled(prefix, style);
        state.push((prefix_span, ToastContent::Pair(from, to)));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a notice with the default prefix associated with the given style.
    pub fn notice(
        style: ToastStyle,
        msg: impl Into<std::borrow::Cow<'static, str>>,
    ) {
        let mut state = TOAST.lock().unwrap();
        let prefix_span = Span::styled(format!("{style}: "), style);
        state.push((prefix_span, ToastContent::Line(msg.into().into())));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a message with empty prefix.
    /// `replace = true` clears all previous messages of this type.
    /// Note: Style the spans, not the line
    pub fn msg(
        line: impl Into<Line<'static>>,
        replace: bool,
    ) {
        let mut state = TOAST.lock().unwrap();

        if replace {
            state.retain(|(prefix, _)| !prefix.content.is_empty());
        }

        let prefix_span = Span::raw("");
        state.push((prefix_span, ToastContent::Line(line.into())));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    pub fn toast_empty() {
        TOAST::msg(
            Span::styled("No entries", Style::new().fg(Color::DarkGray).italic()),
            true,
        );
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
