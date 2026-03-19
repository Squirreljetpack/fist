mod execute;
mod scratch_list;
mod status;
pub use status::*;

use std::{collections::BTreeSet, ffi::OsString, sync::Mutex};

use cba::bath::{PathExt, auto_dest_for_src};
use indexmap::IndexMap;

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    config::{StashAddRule, StashMode},
    run::{
        stash::scratch_list::ScratchList,
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
    pub scratch: Vec<(String, ScratchList)>,
    pub current_scratch: usize,
    pub modes: Vec<(String, StashMode)>,
}

impl StashState {
    pub const fn new() -> Self {
        Self {
            shared: Vec::new(),
            scratch: Vec::new(),
            current_scratch: 0,
            modes: Vec::new(),
        }
    }
}

pub static STASH_STATE: Mutex<StashState> = Mutex::new(StashState::new());

pub static STASH_ACTION_HISTORY: Mutex<Vec<StashItem>> = Mutex::new(Vec::new());

pub static STASH_BUILTINS: [&str; 6] = ["app", "copy", "paste", "cut", "revert", "symlink"];
pub struct STASH;

impl STASH {
    pub fn init(modes: Vec<(String, StashMode)>) {
        let mut state = STASH_STATE.lock().unwrap();
        state.modes = modes;

        // Ensure app and revert are always present and at index 0 and 1
        state.scratch = vec![
            ("app".to_string(), ScratchList::Map(IndexMap::new())),
            ("revert".to_string(), ScratchList::Map(IndexMap::new())),
        ];

        let modes_clone = state.modes.clone();
        for (kind, mode) in &modes_clone {
            if mode.scratch && kind != "app" && kind != "revert" {
                state.scratch.push((
                    kind.clone(),
                    match mode.unique {
                        StashAddRule::False => ScratchList::Vec(Vec::new(), 0),
                        StashAddRule::True => ScratchList::Map(IndexMap::new()),
                        StashAddRule::Limit(n) => ScratchList::Vec(Vec::new(), n),
                    },
                ));
            }
        }
    }

    /// Does not retrieve mode for builtins
    pub fn get_mode(kind: &str) -> StashMode {
        let state = STASH_STATE.lock().unwrap();
        state
            .modes
            .iter()
            .find(|(k, _)| k == kind)
            .map(|(_, m)| m.clone())
            .unwrap_or_default()
    }

    pub fn is_scratch(kind: &str) -> bool {
        if kind == "app" || kind == "revert" {
            return true;
        }
        STASH::get_mode(kind).scratch
    }

    pub fn has_target(kind: &str) -> bool {
        if kind == "app" {
            return false;
        }
        if kind == "copy" || kind == "cut" || kind == "symlink" || kind == "revert" {
            return false;
        }
        STASH::get_mode(kind).target
    }

    pub fn is_unique(kind: &str) -> bool {
        STASH::get_mode(kind).unique.is_true()
    }

    // -----------------------------

    pub fn current_scratch() -> String {
        let state = STASH_STATE.lock().unwrap();
        state.scratch[state.current_scratch].0.clone()
    }

    pub fn scratch_title() -> String {
        let state = STASH_STATE.lock().unwrap();
        let val = &state.scratch[state.current_scratch].0;
        if val == "app" {
            "App (To open)".to_string()
        } else {
            val.clone()
        }
    }

    pub fn set_scratch(kind: &str) -> bool {
        let mut state = STASH_STATE.lock().unwrap();
        if let Some(pos) = state.scratch.iter().position(|(k, _)| k == kind) {
            state.current_scratch = pos;
            true
        } else {
            false
        }
    }

    pub fn cycle_scratch(forwards: bool) {
        let mut state = STASH_STATE.lock().unwrap();

        let len = state.scratch.len();
        if len == 0 {
            return;
        }

        if forwards {
            state.current_scratch = (state.current_scratch + 1) % len;
        } else {
            state.current_scratch = (state.current_scratch + len - 1) % len;
        }
    }

    // -----------------------------------------

    pub fn extend(
        kind: &str,
        items: impl IntoIterator<Item = AbsPath>,
    ) {
        let mut state = STASH_STATE.lock().unwrap();
        let mode = STASH::get_mode(kind);

        for path in items {
            if mode.scratch {
                let list = if let Some(pos) = state.scratch.iter().position(|(k, _)| k == kind) {
                    &mut state.scratch[pos].1
                } else {
                    state.scratch.push((
                        kind.to_string(),
                        match mode.unique {
                            StashAddRule::False => ScratchList::Vec(Vec::new(), 0),
                            StashAddRule::True => ScratchList::Map(IndexMap::new()),
                            StashAddRule::Limit(n) => ScratchList::Vec(Vec::new(), n),
                        },
                    ));
                    &mut state.scratch.last_mut().unwrap().1
                };
                list.push(path, OsString::new());
            } else {
                match mode.unique {
                    StashAddRule::False => {
                        state.shared.push(StashItem::new(kind.to_string(), path));
                    }
                    StashAddRule::True => {
                        if !state.shared.iter().any(|s| s.src == path && s.kind == kind) {
                            state.shared.push(StashItem::new(kind.to_string(), path));
                        }
                    }
                    StashAddRule::Limit(n) => {
                        if let Some(i) = state
                            .shared
                            .iter()
                            .position(|s| s.src == path && s.kind == kind)
                        {
                            state.shared.remove(i);
                            state.shared.push(StashItem::new(kind.to_string(), path));
                        } else {
                            let count = state.shared.iter().filter(|s| s.kind == kind).count();
                            if count >= n as usize && n > 0 {
                                if let Some(i) = state.shared.iter().rposition(|s| s.kind == kind) {
                                    state.shared.remove(i);
                                }
                            }
                            state.shared.push(StashItem::new(kind.to_string(), path));
                        }
                    }
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
        scratch: bool,
        index: usize,
    ) -> Option<(AbsPath, OsString)> {
        let state = STASH_STATE.lock().unwrap();
        if scratch {
            state.scratch[state.current_scratch].1.get(index)
        } else {
            state
                .shared
                .get(index)
                .map(|item| (item.src.clone(), item.dst.clone()))
        }
    }

    pub fn update(
        scratch: bool,
        index: usize,
        path: Option<AbsPath>,
        dst: Option<OsString>,
    ) {
        let mut state = STASH_STATE.lock().unwrap();

        if scratch {
            let idx = state.current_scratch;
            state.scratch[idx].1.update(index, path, dst);
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
        scratch: bool,
        i: usize,
        j: usize,
    ) {
        let mut state = STASH_STATE.lock().unwrap();
        if scratch {
            let idx = state.current_scratch;
            state.scratch[idx].1.swap(i, j);
        } else {
            state.shared.swap(i, j);
        }
    }

    pub fn remove(
        scratch: bool,
        index: usize,
    ) {
        let mut state = STASH_STATE.lock().unwrap();
        if scratch {
            let idx = state.current_scratch;
            state.scratch[idx].1.remove(index);
        } else {
            if index < state.shared.len() {
                state.shared.remove(index);
            }
        }
    }

    // ------------ execute -----------------

    pub fn execute(
        scratch: bool,
        index: usize,
    ) {
        let state = STASH_STATE.lock().unwrap();

        if scratch {
            STASH::execute_all_scratch_impl(Some(&BTreeSet::from([index])));
        } else {
            if let Some(item) = state.shared.get(index).cloned() {
                let mut item = item;
                item.dst = GLOBAL::with_cfg(|c| {
                    auto_dest_for_src(&item.src, &item.dst, &c.fs.rename_policy)
                })
                .into();
                TASKS::spawn_blocking(move || item.execute());
            }
        }
    }

    /// Execute with STACK::nav_cwd() as base
    pub fn execute_all(
        scratch: bool,
        indices: &BTreeSet<usize>,
    ) {
        if scratch {
            STASH::execute_all_scratch_impl(Some(indices));
        } else if let Some(base) = STACK::nav_cwd() {
            STASH::execute_all_impl(base, false, Some(indices));
        } else {
            TOAST::notice(
                ToastStyle::Error,
                "The stack must be executed in a Nav pane.",
            );
        }
    }

    // ------------- clear --------------

    /// Clear items that are not in progress
    pub fn clear(kind: Option<&str>) {
        let mut state = STASH_STATE.lock().unwrap();
        match kind {
            None => state.shared.clear(),
            Some("") => {
                let idx = state.current_scratch;
                state.scratch[idx].1.clear()
            }
            Some(k) if STASH::is_scratch(k) => {
                if STASH::is_scratch(k) {
                    if let Some((_, list)) = state.scratch.iter_mut().find(|(kind, _)| kind == k) {
                        list.clear()
                    }
                }
            }
            _ => {
                state.shared.retain(|item| {
                    kind.is_some_and(|k| k != item.kind) || item.status.state.is_started()
                });
            }
        }
    }

    /// Clear shared items that are complete
    pub fn clear_completed_shared() {
        let mut state = STASH_STATE.lock().unwrap();
        state.shared.retain(|item| !item.status.state.is_complete());
    }
    pub fn clear_completed_scratch() {
        // todo: state tracking for the scratch stash
    }

    // --------------- other ----------------

    pub fn stashed_apps() -> Vec<std::ffi::OsString> {
        let state = STASH_STATE.lock().unwrap();
        state.scratch[0]
            .1
            .as_slice()
            .into_iter()
            .map(|(p, _)| p.to_os_string())
            .collect()
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
