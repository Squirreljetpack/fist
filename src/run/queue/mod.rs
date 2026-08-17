//! Queue state, types, and operations.
//!
//! Defines the core types ([`QueueItem`], [`QueueState`], [`QueueSelector`],
//! [`QueueView`]) and the [`QUEUE`] namespace struct that exposes all queue
//! operations: enqueueing, view CRUD, selector-based dispatch, and clearing.
//! The two module-level statics [`QUEUE_STATE`] and [`QUEUE_ACTION_HISTORY`]
//! are the sole owners of live queue data.

mod execute;
mod status;
pub use status::*;

use std::{ffi::OsString, path::PathBuf, sync::Mutex};

use cba::bath::{PathExt, auto_dest_for_src};

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    run::{
        FsPane,
        state::{GLOBAL, MENU_ACTIONS, STACK, TASKS, TOAST},
    },
};

/// The kind of a queued operation: a builtin (`copy`, `move`, `symlink`,
/// `none`) or a custom menu-action key.
pub type QueueKind = String;

/// The builtin queue kinds, enqueued one row per path.
pub const BUILTIN_KINDS: [&str; 4] = ["copy", "move", "symlink", "none"];

/// The builtin queue kinds that require a destination to execute.
pub const DEST_KINDS: [&str; 3] = ["copy", "move", "symlink"];

/// Selector over queued operations used by `ExecuteQueue`/`ClearQueue`.
/// Parsing is ASCII case-insensitive for the reserved spellings; any other
/// value (including the empty string) is a custom [`QueueKind`].
#[derive(Debug, Clone, PartialEq, Eq, strum_macros::EnumString, strum_macros::Display)]
#[strum(ascii_case_insensitive)]
pub enum QueueSelector {
    /// Every queue kind.
    All,
    /// The builtin transfer kinds (`copy`, `move`, `symlink`).
    Builtins,
    /// The first pending row.
    First,
    /// The last pending row.
    Last,
    /// A specific queue kind.
    #[strum(default, to_string = "{0}")]
    Kind(QueueKind),
}

impl QueueSelector {
    /// Returns the custom or builtin kind string if this is `QueueSelector::Kind`.
    pub fn as_kind(&self) -> Option<&str> {
        match self {
            Self::Kind(k) => Some(k.as_str()),
            _ => None,
        }
    }
}

/// Whether a string is a valid queue category (non-empty and not a reserved selector keyword).
pub fn is_valid_queue_kind(kind: &str) -> bool {
    !kind.is_empty() && matches!(kind.parse::<QueueSelector>(), Ok(QueueSelector::Kind(_)))
}

/// Shared validation function for runtime execution and `fs :tool check`.
pub fn validate_queue_kind(
    kind: &str,
    actions: Option<&crate::menu::MenuActions>,
) -> Result<(), String> {
    if !is_valid_queue_kind(kind) {
        return Err(format!("Unknown queue kind: {kind}"));
    }
    if BUILTIN_KINDS.contains(&kind) {
        return Ok(());
    }
    let is_custom = match actions {
        Some(acts) => acts.contains_key(kind),
        None => crate::run::state::MENU_ACTIONS
            .get()
            .is_some_and(|m| m.contains_key(kind)),
    };
    if is_custom {
        Ok(())
    } else {
        Err(format!("Unknown queue kind: {kind}"))
    }
}

/// Outcome of matching a selector against the pending shared rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorResult {
    /// No pending rows match.
    NoItems,
    /// Matching rows exist but destination-requiring ones cannot execute.
    MissingDestination,
    /// The concrete shared indices ready to execute.
    Ready(Vec<usize>),
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub kind: QueueKind,
    pub src: Vec<AbsPath>,
    pub status: QueueItemStatus,
    pub dst: OsString,
}

impl QueueItem {
    pub fn new(
        kind: QueueKind,
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
    /// Items added under a queue kind (`copy`/`move`/`symlink` builtins, or a
    /// custom kind which executes as a lua script — see [`QueueItem::execute`]).
    pub shared: Vec<QueueItem>,
    /// Not implemented anywhere — kept inert as specified by STASHPANE.md.
    pub revert: Vec<(usize, PathBuf)>,
    /// Currently inert — nothing consumes it.
    pub mode: usize,
}

impl QueueState {
    pub const fn new() -> Self {
        Self {
            shared: Vec::new(),
            revert: Vec::new(),
            mode: 0,
        }
    }

    /// The underlying shared indices visible under the given kind filter.
    pub fn visible_indices(
        &self,
        kind: Option<&str>,
    ) -> Vec<usize> {
        self.shared
            .iter()
            .enumerate()
            .filter(|(_, item)| kind.is_none_or(|k| item.kind == k))
            .map(|(i, _)| i)
            .collect()
    }

    /// The screen position of a queue item row under the given kind filter.
    pub fn visible_position_of(
        &self,
        kind: Option<&str>,
        row: usize,
    ) -> Option<usize> {
        self.visible_indices(kind)
            .into_iter()
            .position(|idx| idx == row)
    }

    /// Cycle the kind filter (+1 / -1) through `[None, distinct kinds...]` with wrapping.
    pub fn next_kind(
        &self,
        current: Option<&str>,
        delta: i32,
    ) -> Option<String> {
        let mut kinds: Vec<&str> = Vec::new();
        for item in &self.shared {
            if !kinds.contains(&item.kind.as_str()) {
                kinds.push(item.kind.as_str());
            }
        }
        let total = kinds.len() + 1;
        if total < 2 {
            return None;
        }
        let pos = match current {
            None => 0,
            Some(c) => kinds.iter().position(|&k| k == c).map(|i| i + 1).unwrap_or(0),
        };
        let next_pos = (pos as i32 + delta).rem_euclid(total as i32) as usize;
        if next_pos == 0 {
            None
        } else {
            Some(kinds[next_pos - 1].to_string())
        }
    }
}

pub static QUEUE_STATE: Mutex<QueueState> = Mutex::new(QueueState::new());

pub static QUEUE_ACTION_HISTORY: Mutex<Vec<QueueItem>> = Mutex::new(Vec::new());

/// Which list an overlay operation targets: the shared items or the
/// pending files of the current app pane (the app view).
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

    /// Enqueue `paths` into the shared queue under `kind`. Builtin kinds
    /// add one row per path, replacing a pending row with the same source
    /// and kind (moved to the tail); custom menu kinds add one multi-path
    /// row.
    pub fn enqueue(
        kind: QueueKind,
        paths: Vec<AbsPath>,
    ) {
        debug_assert!(
            is_valid_queue_kind(&kind),
            "enqueue kinds must parse as an ordinary queue kind, got {kind:?}"
        );
        if paths.is_empty() {
            return;
        }
        let mut state = QUEUE_STATE.lock().unwrap();
        if BUILTIN_KINDS.contains(&kind.as_str()) {
            for path in paths {
                if let Some(i) = state.shared.iter().position(|s| {
                    s.src.len() == 1
                        && s.src[0] == path
                        && s.kind == kind
                        && s.status.state.load() == QueueItemState::Pending
                }) {
                    state.shared.remove(i);
                }
                state.shared.push(QueueItem::new(kind.clone(), path));
            }
        } else {
            state.shared.push(QueueItem {
                kind,
                status: QueueItemStatus::new(&paths[0]),
                src: paths,
                dst: Default::default(),
            });
        }
    }

    // ------------- view ops --------------

    pub fn view_len(view: QueueView) -> usize {
        match view {
            QueueView::Shared => QUEUE_STATE.lock().unwrap().shared.len(),
            QueueView::Apps => STACK::with_current(|p| match p {
                FsPane::Apps { pending, .. } => pending.len(),
                _ => 0,
            }),
        }
    }

    /// `(src, dst)` for the entry at `index`; app entries have no dst.
    pub fn view_get(
        view: QueueView,
        index: usize,
    ) -> Option<(Vec<AbsPath>, OsString)> {
        match view {
            QueueView::Shared => QUEUE_STATE
                .lock()
                .unwrap()
                .shared
                .get(index)
                .map(|item| (item.src.clone(), item.dst.clone())),
            QueueView::Apps => STACK::with_current(|p| match p {
                FsPane::Apps { pending, .. } => pending
                    .get(index)
                    .cloned()
                    .map(|p| (vec![p], OsString::new())),
                _ => None,
            }),
        }
    }

    /// Update the entry at `index`. App entries only take the path.
    pub fn view_update(
        view: QueueView,
        index: usize,
        path: Option<AbsPath>,
        dst: Option<OsString>,
    ) {
        match view {
            QueueView::Shared => {
                let mut state = QUEUE_STATE.lock().unwrap();
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
                if let Some(p) = path {
                    STACK::with_current_mut(|pane| {
                        if let FsPane::Apps { pending, .. } = pane
                            && let Some(slot) = pending.get_mut(index)
                        {
                            *slot = p;
                        }
                    });
                }
            }
        }
    }

    pub fn view_swap(
        view: QueueView,
        i: usize,
        j: usize,
    ) {
        match view {
            QueueView::Shared => QUEUE_STATE.lock().unwrap().shared.swap(i, j),
            QueueView::Apps => STACK::with_current_mut(|pane| {
                if let FsPane::Apps { pending, .. } = pane
                    && i < pending.len()
                    && j < pending.len()
                {
                    pending.swap(i, j);
                }
            }),
        }
    }

    pub fn view_remove(
        view: QueueView,
        index: usize,
    ) {
        match view {
            QueueView::Shared => {
                let mut state = QUEUE_STATE.lock().unwrap();
                if index < state.shared.len() {
                    state.shared.remove(index);
                }
            }
            QueueView::Apps => STACK::with_current_mut(|pane| {
                if let FsPane::Apps { pending, .. } = pane
                    && index < pending.len()
                {
                    pending.remove(index);
                }
            }),
        }
    }

    // ------------ execute -----------------

    /// Whether a pending row cannot execute because its destination is
    /// missing given the effective navigation directory. Builtin kinds infer
    /// an empty destination from it; custom `requires_dest` kinds do not.
    fn dest_missing(
        item: &QueueItem,
        nav_cwd: Option<&AbsPath>,
    ) -> bool {
        if !item.dst.is_empty() {
            return false;
        }
        match item.kind.as_str() {
            kind if DEST_KINDS.contains(&kind) => nav_cwd.is_none(),
            "none" => false,
            custom => MENU_ACTIONS
                .get()
                .and_then(|actions| actions.get(custom))
                .is_some_and(|action| action.requires_dest),
        }
    }

    /// Match `selector` against the pending shared rows.
    ///
    /// `All` and `Builtins` silently skip rows whose destination is missing;
    /// an exact kind and `First`/`Last` report
    /// [`SelectorResult::MissingDestination`] when their selected work
    /// cannot execute.
    pub fn select(
        selector: &QueueSelector,
        nav_cwd: Option<&AbsPath>,
    ) -> SelectorResult {
        let state = QUEUE_STATE.lock().unwrap();
        let pending: Vec<usize> = state
            .shared
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status.state.is_pending())
            .map(|(i, _)| i)
            .collect();

        match selector {
            QueueSelector::First | QueueSelector::Last => {
                let index = if matches!(selector, QueueSelector::First) {
                    pending.first()
                } else {
                    pending.last()
                };
                match index {
                    Some(&i) if Self::dest_missing(&state.shared[i], nav_cwd) => {
                        SelectorResult::MissingDestination
                    }
                    Some(&i) => SelectorResult::Ready(vec![i]),
                    None => SelectorResult::NoItems,
                }
            }
            QueueSelector::Kind(kind) => {
                let matching: Vec<usize> = pending
                    .iter()
                    .copied()
                    .filter(|&i| state.shared[i].kind == *kind)
                    .collect();
                if matching.is_empty() {
                    SelectorResult::NoItems
                } else if matching
                    .iter()
                    .any(|&i| Self::dest_missing(&state.shared[i], nav_cwd))
                {
                    SelectorResult::MissingDestination
                } else {
                    SelectorResult::Ready(matching)
                }
            }
            QueueSelector::All => {
                let ready: Vec<usize> = pending
                    .iter()
                    .copied()
                    .filter(|&i| !Self::dest_missing(&state.shared[i], nav_cwd))
                    .collect();
                if ready.is_empty() {
                    SelectorResult::NoItems
                } else {
                    SelectorResult::Ready(ready)
                }
            }
            QueueSelector::Builtins => {
                let ready: Vec<usize> = pending
                    .iter()
                    .copied()
                    .filter(|&i| {
                        DEST_KINDS.contains(&state.shared[i].kind.as_str())
                            && !Self::dest_missing(&state.shared[i], nav_cwd)
                    })
                    .collect();
                if ready.is_empty() {
                    SelectorResult::NoItems
                } else {
                    SelectorResult::Ready(ready)
                }
            }
        }
    }

    /// Execute the shared items at `indices` against the effective
    /// navigation directory. Rows that are already started are skipped; no
    /// pending filtering happens here — callers select the rows.
    pub fn dispatch(
        indices: Vec<usize>,
        nav_cwd: Option<AbsPath>,
    ) {
        let queue: Vec<QueueItem> = {
            let state = QUEUE_STATE.lock().unwrap();
            indices
                .iter()
                .filter_map(|&i| state.shared.get(i).cloned())
                .filter(|item| !item.status.state.is_started())
                .collect()
        };
        if queue.is_empty() {
            return;
        }

        TOAST::msg(format!("Starting {} items.", queue.len()), true);

        let rename_policy = GLOBAL::cfg().fs.rename_policy.clone();

        TASKS::spawn_blocking(move || {
            for mut item in queue {
                // single-path items resolve their destination here against
                // the effective navigation directory; multi-path items pass
                // their stored destination to `QueueItem::execute` verbatim
                if item.src.len() == 1 {
                    let base_dest: OsString = match (item.dst.is_empty(), nav_cwd.as_ref()) {
                        (true, Some(base)) => {
                            let mut d: OsString = base.as_os_str().to_owned();
                            d.push(std::path::MAIN_SEPARATOR_STR);
                            d
                        }
                        (false, Some(base)) => item.dst.abs(base).into(),
                        _ => item.dst.clone(),
                    };
                    item.dst = auto_dest_for_src(&item.src[0], &base_dest, &rename_policy).into();
                }
                item.execute(nav_cwd.as_ref());
            }
        });
    }

    // ------------- clear --------------

    /// Whether `kind` is covered by `selector`.
    fn kind_matches(
        selector: &QueueSelector,
        kind: &str,
    ) -> bool {
        match selector {
            QueueSelector::All => true,
            QueueSelector::Builtins => DEST_KINDS.contains(&kind),
            QueueSelector::First | QueueSelector::Last => true,
            QueueSelector::Kind(k) => k == kind,
        }
    }

    /// Clear the pending rows matching `selector`; returns whether any were
    /// removed. `First`/`Last` clear a single row.
    pub fn clear_selected(selector: &QueueSelector) -> bool {
        let mut state = QUEUE_STATE.lock().unwrap();
        let indices: Vec<usize> = state
            .shared
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.status.state.is_pending() && Self::kind_matches(selector, &item.kind)
            })
            .map(|(i, _)| i)
            .collect();
        let indices: Vec<usize> = match selector {
            QueueSelector::First => indices.first().copied().into_iter().collect(),
            QueueSelector::Last => indices.last().copied().into_iter().collect(),
            _ => indices,
        };
        if indices.is_empty() {
            return false;
        }
        for i in indices.into_iter().rev() {
            state.shared.remove(i);
        }
        true
    }

    /// Clear shared items that are complete.
    pub fn clear_completed_shared() {
        let mut state = QUEUE_STATE.lock().unwrap();
        state.shared.retain(|item| !item.status.state.is_complete());
    }

    // --------------- other ----------------
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

#[cfg(test)]
mod selector_tests {
    use super::*;

    #[test]
    fn selector_parsing() {
        assert_eq!(
            "".parse::<QueueSelector>().unwrap(),
            QueueSelector::Kind(String::new())
        );
        assert_eq!("all".parse::<QueueSelector>().unwrap(), QueueSelector::All);
        assert_eq!("ALL".parse::<QueueSelector>().unwrap(), QueueSelector::All);
        assert_eq!(
            "builtins".parse::<QueueSelector>().unwrap(),
            QueueSelector::Builtins
        );
        assert_eq!(
            "FIRST".parse::<QueueSelector>().unwrap(),
            QueueSelector::First
        );
        assert_eq!(
            "Last".parse::<QueueSelector>().unwrap(),
            QueueSelector::Last
        );
        // builtin kind names are ordinary kinds, not reserved selectors
        assert_eq!(
            "copy".parse::<QueueSelector>().unwrap(),
            QueueSelector::Kind("copy".into())
        );
        // custom keys preserve their case
        assert_eq!(
            "My-Action".parse::<QueueSelector>().unwrap(),
            QueueSelector::Kind("My-Action".into())
        );
    }

    #[test]
    fn selector_display() {
        assert_eq!(QueueSelector::All.to_string(), "All");
        assert_eq!(QueueSelector::Builtins.to_string(), "Builtins");
        assert_eq!(QueueSelector::First.to_string(), "First");
        assert_eq!(QueueSelector::Kind("k".into()).to_string(), "k");
    }

    #[test]
    fn selector_as_kind() {
        assert_eq!(QueueSelector::All.as_kind(), None);
        assert_eq!(QueueSelector::Builtins.as_kind(), None);
        assert_eq!(QueueSelector::First.as_kind(), None);
        assert_eq!(QueueSelector::Last.as_kind(), None);
        assert_eq!(QueueSelector::Kind("copy".into()).as_kind(), Some("copy"));
        assert_eq!(QueueSelector::Kind("zip".into()).as_kind(), Some("zip"));
    }

    #[test]
    fn test_is_valid_queue_kind() {
        assert!(!is_valid_queue_kind(""));
        assert!(!is_valid_queue_kind("all"));
        assert!(!is_valid_queue_kind("ALL"));
        assert!(!is_valid_queue_kind("builtins"));
        assert!(!is_valid_queue_kind("first"));
        assert!(!is_valid_queue_kind("last"));

        assert!(is_valid_queue_kind("copy"));
        assert!(is_valid_queue_kind("move"));
        assert!(is_valid_queue_kind("symlink"));
        assert!(is_valid_queue_kind("none"));
        assert!(is_valid_queue_kind("zip"));
        assert!(is_valid_queue_kind("my-action"));
    }

    #[test]
    fn test_validate_queue_kind() {
        // Builtins pass with or without custom actions map
        assert!(validate_queue_kind("copy", None).is_ok());
        assert!(validate_queue_kind("move", None).is_ok());
        assert!(validate_queue_kind("symlink", None).is_ok());
        assert!(validate_queue_kind("none", None).is_ok());

        // Reserved selectors fail
        assert_eq!(
            validate_queue_kind("all", None).unwrap_err(),
            "Unknown queue kind: all"
        );
        assert_eq!(
            validate_queue_kind("ALL", None).unwrap_err(),
            "Unknown queue kind: ALL"
        );
        assert_eq!(
            validate_queue_kind("builtins", None).unwrap_err(),
            "Unknown queue kind: builtins"
        );
        assert_eq!(
            validate_queue_kind("first", None).unwrap_err(),
            "Unknown queue kind: first"
        );
        assert_eq!(
            validate_queue_kind("last", None).unwrap_err(),
            "Unknown queue kind: last"
        );
        assert_eq!(
            validate_queue_kind("", None).unwrap_err(),
            "Unknown queue kind: "
        );

        // Unknown custom action without map or global state
        assert_eq!(
            validate_queue_kind("unknown", None).unwrap_err(),
            "Unknown queue kind: unknown"
        );

        // Custom action with provided actions map
        let actions: crate::menu::MenuActions = toml::from_str(
            r#"
            [zip]
            command = "print('zip')"
            strategy = "Queue"
            "#,
        )
        .unwrap();

        assert!(validate_queue_kind("zip", Some(&actions)).is_ok());
        assert_eq!(
            validate_queue_kind("rar", Some(&actions)).unwrap_err(),
            "Unknown queue kind: rar"
        );
    }
}
