#![allow(clippy::upper_case_acronyms)]

use std::{
    cell::RefCell,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use cli_boilerplate_automation::bait::ResultExt;
use log::debug;
use matchmaker::{action::Action, event::RenderSender};
use ratatui::{
    style::Style,
    text::{Line, Span},
};
use tokio;

use crate::config::GlobalConfig;
use crate::{
    abspath::AbsPath,
    cli::env::EnvOpts,
    db::{Connection, DbSortOrder, Pool, zoxide::DbFilter},
    errors::DbError,
    run::{FsPane, action::FsAction, item::PathItem},
    ui::menu_overlay::PromptKind,
    utils::text::{ToastContent, ToastStyle, make_toast},
    watcher::{WatcherMessage, WatcherSender},
};

mod filters;
pub use filters::*;
mod stack;
pub use stack::*;

// ------------- TRACKING -----------------------
thread_local! {
    static PREV_DIRECTORY: RefCell<Option<AbsPath>> = const { RefCell::new(None) };
    static STASHED_INDEX: RefCell<Option<u32>> = const { RefCell::new(None) };
    static INPUT_BAR_CONTENT: RefCell<(Option<PromptKind>, Result<PathItem, AbsPath>)> = const { RefCell::new((None, Err(AbsPath::empty()))) };
    static ORIGINAL_RELATIVE_PATH: RefCell<Option<bool>> = const { RefCell::new(None) };
}
static TEMP_BOOL: AtomicBool = AtomicBool::new(false);
pub struct TEMP {}
impl TEMP {
    pub fn take_prev_dir() -> Option<AbsPath> {
        PREV_DIRECTORY.with_borrow_mut(|x| x.take())
    }
    pub fn set_prev_dir(path: Option<AbsPath>) {
        PREV_DIRECTORY.replace(path);
    }
    pub fn take_stashed_index() -> Option<u32> {
        STASHED_INDEX.with_borrow_mut(|i| i.take())
    }
    pub fn set_stashed_index(index: u32) -> Option<u32> {
        STASHED_INDEX.replace(Some(index))
    }

    pub fn take_input_bar() -> (Option<PromptKind>, Result<PathItem, AbsPath>) {
        INPUT_BAR_CONTENT
            .with_borrow_mut(|(p, s)| (p.take(), std::mem::replace(s, Err(AbsPath::empty()))))
    }

    /// If menu_prompt is set, menu starts an input overlay.
    ///
    /// The Ok variant of menu_target describes the target,
    /// while the Err variant corresponds to no target
    /// -- instead defining the cwd context, in which case
    /// only a restrictred subset of the menu actions is available.
    ///
    /// # Additional
    /// When the prompt is set and the target is Ok, the target's filename is shown in the title of the input bar.
    #[allow(unused_must_use)]
    pub fn set_input_bar(
        menu_prompt: Option<PromptKind>,
        menu_target: Result<PathItem, AbsPath>,
    ) {
        INPUT_BAR_CONTENT.replace((menu_prompt, menu_target));
    }

    pub fn set_initial_relative_path(relative: bool) {
        ORIGINAL_RELATIVE_PATH.replace(Some(relative));
    }
    pub fn get_original_relative_path() -> Option<bool> {
        ORIGINAL_RELATIVE_PATH.with_borrow(|x| *x)
    }

    pub fn set_that_execute_handler_should_process_cwd() {
        TEMP_BOOL.store(true, Ordering::SeqCst);
    }
    pub fn take_whether_execute_handler_should_process_cwd() -> bool {
        TEMP_BOOL
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

// ------------- READ_ONLY ------------------------
thread_local! {
    static CONFIG: RefCell<Option<GlobalConfig>> = const { RefCell::new(None) };
    static WATCHER_TX: RefCell<Option<WatcherSender>> = const { RefCell::new(None) };
    static DB: RefCell<Option<Pool>> = const { RefCell::new(None) };
}
static RENDER_TX: Mutex<Option<RenderSender<FsAction>>> = const { Mutex::new(None) };
// just try different kinds of locks :p
pub static DB_FILTER: tokio::sync::Mutex<Option<DbFilter>> =
    const { tokio::sync::Mutex::const_new(None) };
static ENV_OPTS: LazyLock<Option<EnvOpts>> = LazyLock::new(EnvOpts::init);
pub struct GLOBAL {}
impl GLOBAL {
    /// All global methods can be called iff this has been called
    /// DB_FILTER needs to be initialized seperately with async
    pub fn init(
        cfg: GlobalConfig,
        render_tx: RenderSender<FsAction>,
        watcher_tx: WatcherSender,
        db_pool: Pool,
        pane: FsPane,
    ) {
        // need to handle the patterns listened on by sync_handler
        let sort = match &pane {
            FsPane::Nav { sort, .. }
            | FsPane::Custom { sort, .. }
            | FsPane::Fd { sort, .. }
            | FsPane::Stream { sort, .. } => *sort,
            FsPane::Folders { sort, .. } | FsPane::Files { sort, .. } => (*sort).into(),
            _ => Default::default(),
        };
        let visibility = match &pane {
            FsPane::Nav { vis, .. }
            | FsPane::Custom { vis, .. }
            | FsPane::Fd { vis, .. }
            | FsPane::Stream { vis, .. }
            | FsPane::Rg { vis, .. } => *vis,
            _ => Default::default(),
        };
        debug!("Initial filters: {sort}, {visibility:?}");
        FILTERS::set(sort, visibility);

        CONFIG.with(|c| *c.borrow_mut() = Some(cfg));
        *RENDER_TX.lock().unwrap() = Some(render_tx);
        WATCHER_TX.with(|tx| *tx.borrow_mut() = Some(watcher_tx));
        DB.with(|d| *d.borrow_mut() = Some(db_pool));
        STACK::init(pane);
    }

    /// must be called in initializing thread
    pub fn with_cfg<F, R>(f: F) -> R
    where
        F: FnOnce(&GlobalConfig) -> R,
    {
        CONFIG.with(|c| f(c.borrow().as_ref().unwrap()))
    }

    pub fn with_env<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&EnvOpts) -> Option<R>,
    {
        ENV_OPTS.as_ref().and_then(f)
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
        WATCHER_TX.with(|tx| {
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
        let guard = DB_FILTER.lock().await;
        let db_filter = guard.as_ref().unwrap();
        conn.get_entries(sort, db_filter).await
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

        debug!("{state:?}");

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }

    /// Push a notice with the default prefix associated with the given style
    pub fn push_notice(
        style: ToastStyle,
        msg: impl Into<std::borrow::Cow<'static, str>>,
    ) {
        let mut state = TOAST.lock().unwrap();
        let prefix_span = Span::styled(format!("{style}: "), style);
        state.push((prefix_span, ToastContent::Line(msg.into().into())));

        let toast = make_toast(&state);
        GLOBAL::send_action(FsAction::set_footer(toast));
    }
    /// Push a pair of items a -> b, described by a prefix
    pub fn push_pair(
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

    /// Push a message with empty prefix.
    /// `replace = true` clears all previous messages of this type.
    pub fn push_msg(
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
}

// -----------------------------------------

#[allow(non_snake_case)]
pub mod APP {
    use std::{
        ffi::OsString,
        sync::{Mutex, atomic::AtomicBool},
    };

    use crate::run::{FsPane, state::STACK};

    pub static TO_OPEN: Mutex<Vec<OsString>> = const { Mutex::new(Vec::new()) };
    pub fn in_app_pane() -> bool {
        STACK::with_current(|x| matches!(x, FsPane::Launch { .. }))
    }
    /// ensure recache isn't run more than once
    pub static RAN_RECACHE: AtomicBool = const { AtomicBool::new(false) };
}

// -------------------------------------------
#[allow(non_snake_case)]
pub mod TASKS {
    use std::{cell::RefCell, time::Duration};

    use cli_boilerplate_automation::{dbog, ibog, wbog};
    use tokio::{self, task::JoinSet};

    thread_local! {
        static TASKS: RefCell<JoinSet<()>> = RefCell::new(JoinSet::new());
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
        warn_secs: u64,
        max_secs: u64,
    ) {
        let mut join_set = TASKS.with(|tasks| std::mem::take(&mut *tasks.borrow_mut()));

        if !join_set.is_empty() {
            dbog!("Waiting on {} tasks.", join_set.len());
        }

        let mut remaining = 0;

        let max = tokio::time::sleep(Duration::from_secs(max_secs));
        tokio::pin!(max);

        let start = tokio::time::Instant::now();

        let mut warn = tokio::time::interval_at(
            start + Duration::from_secs(warn_secs),
            Duration::from_secs(warn_secs),
        );

        loop {
            tokio::select! {
                res = join_set.join_next() => {
                    if res.is_none() {
                        break;
                    }
                }

                _ = warn.tick() => {
                    if remaining == 0 {
                        wbog!(
                            "Waiting on {} task(s). (Press ctrl-c to exit).",
                            join_set.len()
                        );
                    } else if join_set.len() != remaining {
                        ibog!(
                            "{} task(s) remaining.",
                            join_set.len()
                        );
                    }
                    remaining = join_set.len()
                }

                _ = &mut max => {
                    wbog!(
                        "Timeout";
                        "{} task(s) aborted.",
                        join_set.len()
                    );
                    break;
                }


            }
        }
    }
}
