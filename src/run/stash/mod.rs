mod exclusive_list;
mod execute;
mod status;
pub use status::*;

use std::{collections::BTreeSet, ffi::OsString, sync::LazyLock, sync::Mutex};

use cba::bath::{PathExt, auto_dest_for_src};
use indexmap::IndexMap;

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    run::{
        stash::exclusive_list::ExclusiveList,
        state::{GLOBAL, STACK, TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

#[derive(Debug, Clone)]
pub struct StashItem {
    pub kind: String,
    pub src: AbsPath,
    pub status: StashItemStatus,
    pub dst: OsString,
}

impl StashItem {
    pub fn new(
        kind: String,
        src: AbsPath,
    ) -> Self {
        Self {
            kind,
            status: StashItemStatus::new(&src),
            src,
            dst: Default::default(),
        }
    }

    pub fn display(&self) -> String {
        self.src.display_short(__home())
    }
}

// -------- GLOBAL ---------

pub struct StashState {
    pub shared: Vec<StashItem>,
    pub exclusive: IndexMap<String, ExclusiveList>,
    pub current_exclusive: String,
}

impl StashState {
    pub fn new() -> Self {
        Self {
            shared: Vec::new(),
            exclusive: IndexMap::new(),
            current_exclusive: "app".to_string(),
        }
    }
}

pub static STASH_STATE: LazyLock<Mutex<StashState>> = LazyLock::new(|| {
    Mutex::new(StashState {
        shared: Vec::new(),
        exclusive: IndexMap::new(),
        current_exclusive: "app".to_string(),
    })
});

pub static STASH_ACTION_HISTORY: Mutex<Vec<StashItem>> = Mutex::new(Vec::new());

pub struct STASH;

impl STASH {
    pub fn is_exclusive(kind: &str) -> bool {
        if kind == "app" || kind == "revert" {
            return true;
        }
        GLOBAL::with_cfg(|c| {
            c.stash
                .modes
                .get(kind)
                .map(|m| m.exclusive)
                .unwrap_or(false)
        })
    }

    pub fn has_target(kind: &str) -> bool {
        GLOBAL::with_cfg(|c| c.stash.modes.get(kind).map(|m| m.target).unwrap_or(false))
    }

    pub fn is_unique(kind: &str) -> bool {
        GLOBAL::with_cfg(|c| c.stash.modes.get(kind).map(|m| m.unique).unwrap_or(true))
    }

    // -----------------------------

    pub fn current_exclusive() -> String {
        STASH_STATE.lock().unwrap().current_exclusive.clone()
    }

    pub fn set_exclusive(kind: String) {
        STASH_STATE.lock().unwrap().current_exclusive = kind;
    }

    pub fn cycle_exclusive(forwards: bool) {
        let mut state = STASH_STATE.lock().unwrap();
        let mut modes: Vec<String> = vec!["app".to_string(), "revert".to_string()];

        for k in state.exclusive.keys() {
            if k != "app" && k != "revert" {
                modes.push(k.clone());
            }
        }

        // Add from config if not present in state
        GLOBAL::with_cfg(|c| {
            for (k, m) in &c.stash.modes {
                if m.exclusive && !modes.contains(k) {
                    modes.push(k.clone());
                }
            }
        });

        if let Some(pos) = modes.iter().position(|m| *m == state.current_exclusive) {
            let len = modes.len();
            let next_pos = if forwards {
                (pos + 1) % len
            } else {
                (pos + len - 1) % len
            };
            state.current_exclusive = modes[next_pos].clone();
        } else {
            state.current_exclusive = "app".to_string();
        }
    }

    // -----------------------------------------

    pub fn extend(
        kind: &str,
        items: impl IntoIterator<Item = AbsPath>,
    ) {
        let mut state = STASH_STATE.lock().unwrap();
        let unique = STASH::is_unique(kind);
        let exclusive = STASH::is_exclusive(kind);

        for path in items {
            if exclusive {
                let list = state.exclusive.entry(kind.to_string()).or_insert_with(|| {
                    if unique {
                        ExclusiveList::Map(IndexMap::new())
                    } else {
                        ExclusiveList::Vec(Vec::new())
                    }
                });
                if !unique || !list.iter_any(&path) {
                    list.push(path, OsString::new());
                } else if unique {
                    if let Some(i) = list.position(&path) {
                        list.remove(i);
                    }
                    list.push(path, OsString::new());
                }
            } else {
                if !unique || !state.shared.iter().any(|s| s.src == path && s.kind == kind) {
                    state.shared.push(StashItem::new(kind.to_string(), path));
                } else if unique {
                    if let Some(i) = state
                        .shared
                        .iter()
                        .position(|s| s.src == path && s.kind == kind)
                    {
                        state.shared.remove(i);
                    }
                    state.shared.push(StashItem::new(kind.to_string(), path));
                }
            }
        }
    }

    pub fn stash(
        kind: &str,
        path: AbsPath,
    ) {
        STASH::extend(kind, std::iter::once(path));
    }

    // ------------- std ops ------------------

    pub fn get(
        exclusive: bool,
        index: usize,
    ) -> Option<(AbsPath, OsString)> {
        let state = STASH_STATE.lock().unwrap();
        if exclusive {
            let kind = state.current_exclusive.clone();
            state.exclusive.get(&kind).and_then(|list| list.get(index))
        } else {
            state
                .shared
                .get(index)
                .map(|item| (item.src.clone(), item.dst.clone()))
        }
    }

    pub fn update(
        exclusive: bool,
        index: usize,
        path: Option<AbsPath>,
        dst: Option<OsString>,
    ) {
        let mut state = STASH_STATE.lock().unwrap();

        if exclusive {
            let kind = STASH::current_exclusive();
            if let Some(list) = state.exclusive.get_mut(&kind) {
                list.update(index, path, dst);
            }
        } else {
            if let Some(item) = state.shared.get_mut(index) {
                if let Some(p) = path {
                    item.src = p;
                }
                if let Some(d) = dst {
                    item.dst = d;
                }
            }
        }
    }

    pub fn swap(
        exclusive: bool,
        i: usize,
        j: usize,
    ) {
        let mut state = STASH_STATE.lock().unwrap();
        if exclusive {
            let kind = state.current_exclusive.clone();
            if let Some(list) = state.exclusive.get_mut(&kind) {
                list.swap(i, j);
            }
        } else {
            state.shared.swap(i, j);
        }
    }

    pub fn remove(
        exclusive: bool,
        index: usize,
    ) {
        let mut state = STASH_STATE.lock().unwrap();
        if exclusive {
            let kind = STASH::current_exclusive();
            if let Some(list) = state.exclusive.get_mut(&kind) {
                list.remove(index);
            }
        } else {
            if index < state.shared.len() {
                state.shared.remove(index);
            }
        }
    }

    // ------------ execute -----------------

    pub fn execute(
        exclusive: bool,
        index: usize,
    ) {
        if exclusive {
            STASH::execute_exclusive(index);
        } else {
            STASH::execute_shared(index);
        }
    }

    /// Execute with STACK::nav_cwd() as base
    pub fn execute_all(
        exclusive: bool,
        indices: &BTreeSet<usize>,
    ) {
        if exclusive {
            STASH::execute_exclusive_all(Some(indices));
        } else if let Some(base) = STACK::nav_cwd() {
            STASH::transfer_all(base, false, Some(indices));
        } else {
            TOAST::notice(
                ToastStyle::Error,
                "The stack must be executed in a Nav pane.",
            );
        }
    }

    pub fn execute_shared(index: usize) {
        let state = STASH_STATE.lock().unwrap();
        if let Some(item) = state.shared.get(index).cloned() {
            let mut item = item;
            item.dst =
                GLOBAL::with_cfg(|c| auto_dest_for_src(&item.src, &item.dst, &c.fs.rename_policy))
                    .into();
            TASKS::spawn_blocking(move || item.transfer());
        }
    }

    pub fn execute_exclusive(index: usize) {
        STASH::execute_exclusive_all(Some(&BTreeSet::from([index])));
    }

    pub fn execute_exclusive_all(indices: Option<&BTreeSet<usize>>) {
        let state = STASH_STATE.lock().unwrap();
        let kind = state.current_exclusive.clone();
        if let Some(list) = state.exclusive.get(&kind) {
            let items: Vec<_> = list
                .as_slice()
                .into_iter()
                .enumerate()
                .filter(|(i, _)| indices.is_none_or(|ids: &BTreeSet<usize>| ids.contains(i)))
                .map(|(_, (src, dst))| {
                    let mut item = StashItem::new(kind.clone(), src);
                    item.dst = dst;
                    item.dst = GLOBAL::with_cfg(|c| {
                        auto_dest_for_src(&item.src, &item.dst, &c.fs.rename_policy)
                    })
                    .into();
                    item
                })
                .collect();

            if !items.is_empty() {
                TASKS::spawn_blocking(move || {
                    for item in items {
                        item.transfer();
                    }
                });
            }
        }
    }

    pub fn clear(kind: Option<&str>) {
        let mut state = STASH_STATE.lock().unwrap();
        let current_exclusive = state.current_exclusive.clone();
        match kind {
            None => state.shared.clear(),
            Some("") => {
                if let Some(x) = state.exclusive.get_mut(&current_exclusive) {
                    x.clear()
                }
            }
            Some(k) => {
                if STASH::is_exclusive(k) {
                    if let Some(s) = state.exclusive.get_mut(k) {
                        s.clear()
                    }
                } else {
                    state
                        .shared
                        .retain(|item| item.kind != k || item.status.state.is_started());
                }
            }
        }
    }

    // --------------- other ----------------

    pub fn stashed_apps() -> Vec<std::ffi::OsString> {
        let state = STASH_STATE.lock().unwrap();
        state
            .exclusive
            .get("app")
            .map(|list| {
                list.as_slice()
                    .into_iter()
                    .map(|(p, _)| p.to_os_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl PartialEq for StashItem {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.src == other.src && self.kind == other.kind
    }
}

impl Eq for StashItem {}
