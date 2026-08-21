#![expect(non_snake_case)]
//! Process-global singleton state: the config, database pool, and the event
//! senders used to talk to the renderer, the watcher, and the bind layer.

use std::cell::OnceCell;
use std::sync::OnceLock;

use cba::bait::ResultExt;
use log::debug;
use matchmaker::{
    action::Action,
    event::{BindSender, RenderSender},
    message::BindDirective,
};

use crate::{
    config::GlobalConfig,
    db::{Connection, Pool},
    errors::DbError,
    run::{FsAction, pane::FsPane},
    watcher::{WatcherMessage, WatcherSender},
};
use fist_types::filters::SortOrder;

use super::{DB_FILTER, FILTERS, STACK};

thread_local! {
    static CONFIG: OnceCell<GlobalConfig> = const { OnceCell::new() };
    static BIND_TX: OnceCell<BindSender<FsAction>> = const { OnceCell::new() };
}

fn tls_ref<T>(value: &T) -> &'static T {
    // These values are only accessed on the thread that owns their TLS.
    unsafe { &*(value as *const T) }
}

static DB: OnceLock<Pool> = OnceLock::new();
static RENDER_TX: OnceLock<RenderSender<FsAction>> = OnceLock::new();
static WATCHER_TX: OnceLock<WatcherSender> = OnceLock::new();

/// All global functions can be called iff [`init`] has been called.
/// [`DB_FILTER`] needs to be initialized separately.
pub fn init(
    config: GlobalConfig,
    render_tx: RenderSender<FsAction>,
    watcher_tx: WatcherSender,
    db_pool: Pool,
    pane: FsPane,
    bind_tx: BindSender<FsAction>,
) {
    #[cfg(test)]
    {
        if CONFIG.with(|c| c.get().is_some()) {
            return;
        }
    }

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

    CONFIG.with(|c| c.set(config).ok());
    let _ = DB.set(db_pool);
    let _ = RENDER_TX.set(render_tx);
    let _ = WATCHER_TX.set(watcher_tx);
    let _ = BIND_TX.with(|tx| tx.set(bind_tx));
    STACK::init(pane);
}

/// This lifetime should be 'thread, but there is no such lifetime. Do not pass off thread!
pub fn cfg() -> &'static GlobalConfig {
    CONFIG
        .with(tls_ref)
        .get()
        .expect("GlobalConfig not initialized")
}

/// `#[cfg(test)]` helper: wire the render/watcher/bind senders to dummy
/// channels so tests that emit actions/toasts don't trip the
/// expect-based senders.
#[cfg(test)]
pub fn init_test_senders() {
    let (bind_tx, _bind_rx) = tokio::sync::mpsc::unbounded_channel();
    let (render_tx, _render_rx) =
        tokio::sync::mpsc::unbounded_channel::<matchmaker::message::RenderCommand<FsAction>>();
    let (watcher_tx, _watcher_rx) = tokio::sync::mpsc::unbounded_channel();

    let _ = RENDER_TX.set(render_tx);
    let _ = WATCHER_TX.set(watcher_tx);
    let _ = BIND_TX.with(|tx| tx.set(bind_tx));
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
pub fn db() -> &'static Pool {
    DB.get().expect("GLOBAL::init not called")
}

pub async fn get_db_entries(
    conn: &mut Connection,
    sort: SortOrder,
) -> Result<Vec<crate::db::Entry>, DbError> {
    let config = DB_FILTER.get().expect("DB_FILTER not initialized");
    conn.get_entries(sort, config, conn.table).await
}
