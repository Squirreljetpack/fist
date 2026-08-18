//! Sort state: the single `SORT_MODE` global and the nucleo application points.
//!
//! The pane is the source of truth for its sort; this module only records the
//! *engaged* sort mode and pushes it into nucleo ([`set_sort_in_nucleo`]).

use std::{
    cmp::Ordering,
    path::Path,
    sync::{Arc, LazyLock, Mutex},
    time::UNIX_EPOCH,
};

use fist_types::filters::SortOrder;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        item::PathItem,
        state::{HideMetadata, GLOBAL, STACK, STORE, TASKS},
        FsAction, FsPane,
    },
};

#[derive(Clone, Copy)]
pub struct SortMode {
    pub order: SortOrder,
    pub threshold: u32,
}

// order:    SortOrder::none = no nucleo sort fn (db/rg panes, or the pane sort is none)
//           anything else   = hard sort engaged; while the fresh default sort is
//                             hiding metadata (the HideMetadata marker) the threshold
//                             is the pane stability, else u32::MAX
static SORT_MODE: Mutex<SortMode> = Mutex::new(SortMode {
    order: SortOrder::none,
    threshold: 0,
});

static DIR_SIZE: LazyLock<fist_size::DirSizeCache> = LazyLock::new(fist_size::DirSizeCache::new);

pub fn set_sort(order: SortOrder) {
    SORT_MODE.lock().unwrap().order = order;
}

pub fn get_sort() -> SortMode {
    *SORT_MODE.lock().unwrap()
}

/// Whether the engaged sort is `SortOrder::none` (db/rg panes, or the pane
/// sort is `none`): no nucleo sort fn is engaged.
pub fn order_is_none() -> bool {
    matches!(get_sort().order, SortOrder::none)
}

/// Updates the global `SORT_MODE` lock and returns the new `SortMode`.
fn set_global_sort_only_from_pane() -> SortMode {
    // db/rg panes order by insertion order (SQL/rg already ordered)
    let order = if STACK::reloads_by_sorting() {
        SortOrder::none
    } else {
        STACK::with_current(FsPane::sort_order)
    };
    let threshold = match order {
        SortOrder::none => STACK::with_current(FsPane::stability_threshold),
        // hard sort while a fresh default sort is hiding the metadata column
        // (marker set by fs_reload / the start initializer, consumed by ReSort):
        // engage the pane stability instead of u32::MAX so the match list
        // re-sorts with the configured tolerance
        _ if STORE::get::<HideMetadata>().is_some() => {
            STACK::with_current(FsPane::stability_threshold)
        }
        _ => u32::MAX,
    };
    let mode = SortMode { order, threshold };

    // direct nucleo calls: matchmaker's Worker::set_stability auto-resorts
    *SORT_MODE.lock().unwrap() = mode;

    mode
}

/// Pushes the current pane's sort into global and updates Nucleo.
pub fn set_sort_from_pane(state: &mut MMState<'_>) {
    let SortMode { order, threshold } = set_global_sort_only_from_pane();

    state.picker_ui.worker.nucleo.sort_with(sort_fn_for(order));
    state.picker_ui.worker.nucleo.set_stability(threshold);
}

pub type SortFn = Arc<dyn Fn((u32, &PathItem), (u32, &PathItem)) -> bool + Send + Sync>;
fn sort_fn_for(order: SortOrder) -> Option<SortFn> {
    match order {
        // no sort fn: match-score/insertion order (db/rg panes, sort none)
        SortOrder::none => None,
        // plain name via sort_name(), with insertion-order tie-breaker
        SortOrder::name => Some(Arc::new(|(i, a), (j, b)| {
            match a.sort_name().cmp(&b.sort_name()) {
                Ordering::Equal => i < j,
                other => other.is_lt(),
            }
        })),
        // descending (newest first); unset values stored as 0 sort last; tie-breaker preserves insertion order
        SortOrder::mtime | SortOrder::atime => {
            Some(Arc::new(|(i, a), (j, b)| match b.value().cmp(&a.value()) {
                Ordering::Equal => i < j,
                other => other.is_lt(),
            }))
        }
        // descending (largest first); missing cache entries read as 0 → sort last; tie-breaker preserves insertion order
        SortOrder::size => Some(Arc::new(|(i, a), (j, b)| {
            let sa = size_of(&a.path).unwrap_or(0);
            let sb = size_of(&b.path).unwrap_or(0);
            match sb.cmp(&sa) {
                Ordering::Equal => i < j,
                other => other.is_lt(),
            }
        })),
    }
}

/// Shared accessor over the size cache: the interactive comparator,
/// [`crate::run::start::format_tail`], and `--list` size sorting all read
/// through this one source.
///
/// `Option` (not `unwrap_or(0)`): the cache only stores explicitly-`add`ed
/// paths, so `None` = "not in cache" and `0` = "genuinely empty" must stay
/// distinguishable for display.
pub fn size_of(path: impl AsRef<Path>) -> Option<u64> {
    DIR_SIZE.get_path(path)
}

/// The shared [`fist_size::DirSizeCache`] (see [`size_of`]).
pub fn dir_size() -> &'static fist_size::DirSizeCache {
    &DIR_SIZE
}

/// Drop all cached sizes and cancel any in-flight walks.
///
/// Called when (re-)engaging a size sort so the pane always works on fresh
/// sizes; also cancels stale fills from a previous pane/sort.
pub fn clear_dir_sizes() {
    DIR_SIZE.clear();
}

fn stat_time(
    path: &AbsPath,
    order: SortOrder,
) -> u64 {
    std::fs::metadata(path.inner())
        .ok()
        .and_then(|m| {
            if order == SortOrder::mtime {
                m.modified().ok()
            } else {
                m.accessed().ok()
            }
        })
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Populate-time metadata fill. Called inline before every push; a no-op
/// unless the engaged order needs a value — mtime/atime set the item value,
/// size fills the cache, `name`/`none` do nothing.
pub fn store_sort_value(
    item: &PathItem,
    order: SortOrder,
) {
    match order {
        SortOrder::mtime | SortOrder::atime => {
            item.set_value(stat_time(&item.path, order));
        }
        SortOrder::size => DIR_SIZE.add(item.path.inner()),
        SortOrder::name | SortOrder::none => {}
    }
}

/// Fill flow for a mid-pane sort change (no reload): snapshot the existing
/// items, compute the new sort values off-thread, then dispatch `ReSort`.
pub fn fill_then_resort(state: &MMState<'_>) {
    let SortMode { order, threshold } = set_global_sort_only_from_pane();

    match order {
        // no fill needed: name compares paths, none unsorts
        SortOrder::name | SortOrder::none => {
            GLOBAL::send_action(FsAction::ReSort);
        }
        SortOrder::mtime | SortOrder::atime | SortOrder::size => {
            let items = state.picker_ui.worker.nucleo.items();
            TASKS::spawn_blocking("sort fill", move || {
                match order {
                    SortOrder::mtime | SortOrder::atime => {
                        for (_, item) in items.iter() {
                            if get_sort().order != order {
                                return;
                            }
                            item.set_value(stat_time(&item.path, order))
                        }
                    }
                    SortOrder::size => {
                        clear_dir_sizes();
                        for (_, item) in items.iter() {
                            if get_sort().order != order {
                                return;
                            }
                            DIR_SIZE.add(item.path.inner());
                        }
                        DIR_SIZE.wait();
                    }
                    _ => {}
                }

                GLOBAL::send_action(FsAction::ReSort);
            });
        }
    }
}

/// Post-populate size fill: paths were submitted during injection; wait for
/// the cache, then let `ReSort` apply the values and resort.
pub fn wait_sizes_then_resort() {
    TASKS::spawn_blocking("sort sizes", || {
        DIR_SIZE.wait();
        GLOBAL::send_action(FsAction::ReSort);
    });
}
