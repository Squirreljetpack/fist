mod execute;
mod status;
pub use status::*;

use std::{ffi::OsString, path::PathBuf, sync::Mutex};

use cba::bath::{PathExt, auto_dest_for_src};

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    run::state::{GLOBAL, STACK, TASKS, TOAST, ToastStyle},
    spawn::menu_action::RESERVED_KEYS,
};

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub kind: String,
    pub src: Vec<AbsPath>,
    pub status: QueueItemStatus,
    pub dst: OsString,
}

impl QueueItem {
    pub fn new(
        kind: String,
        src: AbsPath,
    ) -> Self {
        Self {
            kind,
            status: QueueItemStatus::new(&src),
            src: vec![src],
            dst: Default::default(),
        }
    }

    /// One path → its short display; multiple paths → "[n items]"; none → empty.
    pub fn display(&self) -> String {
        match self.src.len() {
            0 => String::new(),
            1 => self.src[0].display_short(__home()),
            n => format!("[{n} items]"),
        }
    }
}

// -------- GLOBAL ---------

/// The shared queue state.
pub struct QueueState {
    /// Items added under a queue kind (`copy`/`cut`/`symlink` builtins, or a
    /// custom kind which executes as a lua script — see [`QueueItem::execute`]).
    pub shared: Vec<QueueItem>,
    /// Paths collected while in the app pane (`fs :open`, `OpenWith`) — the
    /// files the selected program is opened with. Replaces the `"app"`
    /// scratch list.
    pub apps: Vec<PathBuf>,
    /// Not implemented anywhere — kept inert as specified by STASHPANE.md.
    pub revert: Vec<(usize, PathBuf)>,
    /// Currently inert — nothing consumes it.
    pub mode: usize,
}

impl QueueState {
    pub const fn new() -> Self {
        Self {
            shared: Vec::new(),
            apps: Vec::new(),
            revert: Vec::new(),
            mode: 0,
        }
    }
}

pub static QUEUE_STATE: Mutex<QueueState> = Mutex::new(QueueState::new());

pub static QUEUE_ACTION_HISTORY: Mutex<Vec<QueueItem>> = Mutex::new(Vec::new());

/// Which list an overlay operation targets: the shared items or the app
/// paths collected in app mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueView {
    Shared,
    Apps,
}

/// `ShowQueue` dispatches on this: `0` = the shared queue overlay, `1` = the
/// app view (overlay index 1).
pub fn show_queue_variant() -> u8 {
    if STACK::in_app() { 1 } else { 0 }
}

pub struct QUEUE;

impl QUEUE {
    // ------------- insert --------------

    /// Enqueue all `paths` as a single item under `kind` (used by the menu
    /// action Stash/Batch strategies). Custom kinds always add a fresh item;
    /// the builtin kinds never reach this path (see [`QUEUE::extend`]).
    pub fn enqueue(
        kind: &str,
        paths: Vec<AbsPath>,
    ) {
        debug_assert!(
            !RESERVED_KEYS.contains(&kind),
            "enqueue is for custom menu action kinds, got {kind:?}"
        );
        if paths.is_empty() {
            return;
        }
        let mut state = QUEUE_STATE.lock().unwrap();
        state.shared.push(QueueItem {
            kind: kind.to_string(),
            status: QueueItemStatus::new(&paths[0]),
            src: paths,
            dst: Default::default(),
        });
    }

    /// Stash `paths` under `kind`. `"app"` routes to the apps list (dedup by
    /// path); everything else goes to the shared list, replacing an existing
    /// item with the same `src` + `kind` only while it is pending (moving it
    /// to the tail); a non-pending item is left alone and a fresh item is
    /// added.
    pub fn extend(
        kind: &str,
        items: impl IntoIterator<Item = AbsPath>,
    ) {
        debug_assert!(
            RESERVED_KEYS.contains(&kind),
            "extend is for builtin kinds, got {kind:?}"
        );
        let mut state = QUEUE_STATE.lock().unwrap();
        if kind == "app" {
            for path in items {
                let p = path.inner();
                if !state.apps.contains(&p) {
                    state.apps.push(p);
                }
            }
        } else {
            for path in items {
                if let Some(i) = state.shared.iter().position(|s| {
                    s.src.len() == 1
                        && s.src[0] == path
                        && s.kind == kind
                        && s.status.state.load() == QueueItemState::Pending
                }) {
                    state.shared.remove(i);
                }
                state.shared.push(QueueItem::new(kind.to_string(), path));
            }
        }
    }

    pub fn stash(
        kind: &str,
        path: AbsPath,
    ) {
        QUEUE::extend(kind, std::iter::once(path));
    }

    // ------------- view ops --------------

    pub fn view_len(view: QueueView) -> usize {
        let state = QUEUE_STATE.lock().unwrap();
        match view {
            QueueView::Shared => state.shared.len(),
            QueueView::Apps => state.apps.len(),
        }
    }

    /// `(src, dst)` for the entry at `index`; app entries have no dst.
    pub fn view_get(
        view: QueueView,
        index: usize,
    ) -> Option<(Vec<AbsPath>, OsString)> {
        let state = QUEUE_STATE.lock().unwrap();
        match view {
            QueueView::Shared => state
                .shared
                .get(index)
                .map(|item| (item.src.clone(), item.dst.clone())),
            QueueView::Apps => state
                .apps
                .get(index)
                .cloned()
                .map(|p| (vec![AbsPath::new_unchecked(p)], OsString::new())),
        }
    }

    /// Update the entry at `index`. App entries only take the path.
    pub fn view_update(
        view: QueueView,
        index: usize,
        path: Option<AbsPath>,
        dst: Option<OsString>,
    ) {
        let mut state = QUEUE_STATE.lock().unwrap();
        match view {
            QueueView::Shared => {
                if let Some(item) = state.shared.get_mut(index) {
                    if let Some(p) = path {
                        item.src = vec![p];
                    }
                    if let Some(d) = dst {
                        item.dst = d;
                    }
                }
            }
            QueueView::Apps => {
                if let Some(p) = path
                    && let Some(slot) = state.apps.get_mut(index)
                {
                    *slot = p.inner();
                }
            }
        }
    }

    pub fn view_swap(
        view: QueueView,
        i: usize,
        j: usize,
    ) {
        let mut state = QUEUE_STATE.lock().unwrap();
        match view {
            QueueView::Shared => state.shared.swap(i, j),
            QueueView::Apps => state.apps.swap(i, j),
        }
    }

    pub fn view_remove(
        view: QueueView,
        index: usize,
    ) {
        let mut state = QUEUE_STATE.lock().unwrap();
        match view {
            QueueView::Shared => {
                if index < state.shared.len() {
                    state.shared.remove(index);
                }
            }
            QueueView::Apps => {
                if index < state.apps.len() {
                    state.apps.remove(index);
                }
            }
        }
    }

    // ------------ execute -----------------

    /// Execute the shared item at `index` (blocking transfer in a task).
    /// App entries are not executable — they are files to be opened.
    pub fn execute(index: usize) {
        let state = QUEUE_STATE.lock().unwrap();
        if let Some(item) = state.shared.get(index).cloned() {
            let mut item = item;
            // single-path items resolve their destination here; multi-path
            // items (menu Stash/Batch) resolve per path in `QueueItem::execute`
            if item.src.len() == 1 {
                item.dst = GLOBAL::with_cfg(|c| {
                    auto_dest_for_src(&item.src[0], &item.dst, &c.fs.rename_policy)
                })
                .into();
            }
            TASKS::spawn_blocking(move || item.execute());
        }
    }

    /// Execute with STACK::nav_cwd() as base.
    pub fn execute_all(indices: &std::collections::BTreeSet<usize>) {
        if let Some(base) = STACK::nav_cwd() {
            QUEUE::execute_all_impl(base, false, Some(indices));
        } else {
            TOAST::notice(
                ToastStyle::Error,
                "The stack must be executed in a Nav pane.",
            );
        }
    }

    // ------------- clear --------------

    /// Clear the items whose state is selected by `flags`. Started items are
    /// never cleared.
    pub fn clear(flags: QueueItems) {
        let mut state = QUEUE_STATE.lock().unwrap();
        state.shared.retain(|item| {
            let bit = item.status.state.load().to_bitflag();
            bit == QueueItems::Started || !flags.contains(bit)
        });
    }

    /// Clear shared items that are complete.
    pub fn clear_completed_shared() {
        let mut state = QUEUE_STATE.lock().unwrap();
        state.shared.retain(|item| !item.status.state.is_complete());
    }

    // --------------- other ----------------

    /// The files collected in app mode (`fs :open`, `OpenWith`).
    pub fn stashed_apps() -> Vec<OsString> {
        let state = QUEUE_STATE.lock().unwrap();
        state
            .apps
            .iter()
            .map(|p| p.as_os_str().to_os_string())
            .collect()
    }
}

impl PartialEq for QueueItem {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.src == other.src && self.kind == other.kind
    }
}

impl Eq for QueueItem {}
