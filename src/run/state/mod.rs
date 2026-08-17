#![allow(non_snake_case)]
use std::sync::OnceLock;

use crate::db::zoxide::HistoryConfig;
use crate::menu::MenuActions;

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

pub mod global;
pub use global::GLOBAL;
pub mod tasks;
pub use tasks::{TASKS, TaskId};

// ------------- TRACKING -----------------------

pub static DB_FILTER: OnceLock<HistoryConfig> = OnceLock::new();
/// The user's custom menu actions, registered at startup so the execute
/// handlers can look up an action by key (discriminant 7). Read from the queue
/// worker thread too — [`crate::run::queue::QueueItem::execute`] looks up the
/// action's command under `TASKS::spawn_blocking` — so it is a process-global,
/// thread-safe `OnceLock` rather than a thread-local.
pub static MENU_ACTIONS: OnceLock<MenuActions> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::GlobalConfig;
    use crate::db::Pool;
    use crate::run::{FsAction, pane::FsPane};
    use fist_types::filters::SortOrder;

    #[tokio::test]
    async fn test_global_cfg_and_init() {
        let (bind_tx, bind_rx) = tokio::sync::mpsc::unbounded_channel();
        let (render_tx, render_rx) =
            tokio::sync::mpsc::unbounded_channel::<matchmaker::message::RenderCommand<FsAction>>();
        let (watcher_tx, watcher_rx) = tokio::sync::mpsc::unbounded_channel();

        let pool = Pool {
            pool: sqlx::SqlitePool::connect_lazy("sqlite::memory:").unwrap(),
            lambda: None,
        };
        let pane = FsPane::Nav {
            cwd: crate::abspath::AbsPath::new("/tmp"),
            sort: SortOrder::default(),
            vis: fist_types::filters::Visibility::default(),
            input: (String::new(), 0),
            complete: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            depth: 0,
        };

        GLOBAL::init(
            GlobalConfig::default(),
            render_tx,
            watcher_tx,
            pool,
            pane,
            bind_tx,
        );
        // std::mem::forget((bind_rx, render_rx, watcher_rx));

        let cfg: &'static GlobalConfig = GLOBAL::cfg();
        assert_eq!(
            cfg.interface.alt_accept,
            GlobalConfig::default().interface.alt_accept
        );
    }
}
