//! In-memory backing store of transient stash panes.

use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use cba::vecmap::VecMap;

use crate::{
    abspath::AbsPath,
    db::{Epoch, StashEntry},
};

/// Entries of the transient stashes, keyed by stash name. Each stash's
/// entries are stored newest-first, matching the db's `add_time DESC`
/// ordering. The store starts empty each run — transient stashes are
/// cleared by construction, nothing is persisted.
pub static MEM_STASHES: Mutex<VecMap<String, Vec<StashEntry>>> = Mutex::new(VecMap::new());

fn now() -> Epoch {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as Epoch
}

/// All entries of the named transient stash (newest first).
pub fn mem_get(name: &str) -> Vec<StashEntry> {
    MEM_STASHES
        .lock()
        .unwrap()
        .get(name)
        .cloned()
        .unwrap_or_default()
}

/// Whether the named transient stash already contains `path`.
pub fn mem_has(name: &str, path: &AbsPath) -> bool {
    MEM_STASHES
        .lock()
        .unwrap()
        .get(name)
        .is_some_and(|entries| entries.iter().any(|e| e.stash == *path))
}

/// Add `path` as a fresh entry at the front of the named transient stash
/// (newest add time first).
pub fn mem_add(name: &str, path: &AbsPath) {
    MEM_STASHES
        .lock()
        .unwrap()
        .get_or_insert_mut(name.to_string(), Vec::new())
        .insert(
            0,
            StashEntry {
                id: 0,
                name: name.to_string(),
                stash: path.clone(),
                tail: String::new(),
                add_time: now(),
            },
        );
}

/// Remove the entries of the named transient stash whose path is
/// contained in `paths`; returns the number removed.
pub fn mem_remove(name: &str, paths: &[AbsPath]) -> usize {
    let mut mem = MEM_STASHES.lock().unwrap();
    let Some(entries) = mem.get_mut(name) else {
        return 0;
    };
    let before = entries.len();
    entries.retain(|e| !paths.contains(&e.stash));
    before - entries.len()
}

/// Set the tail (alias analogue of the apps pane) of the named transient
/// stash entry whose path matches. Returns whether an entry was updated.
pub fn mem_set_tail(name: &str, path: &AbsPath, tail: &str) -> bool {
    let mut mem = MEM_STASHES.lock().unwrap();
    match mem.get_mut(name) {
        Some(entries) => match entries.iter_mut().find(|e| e.stash == *path) {
            Some(e) => {
                e.tail = tail.to_string();
                true
            }
            None => false,
        },
        None => false,
    }
}