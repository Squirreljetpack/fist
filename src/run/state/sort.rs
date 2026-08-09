//! Sort state: the single `SORT_MODE` global and the nucleo application points.
//!
//! The pane is the source of truth for its sort; this module only records the
//! *engaged* sort mode and pushes it into nucleo ([`update_sort`]).

use std::sync::{Arc, LazyLock, Mutex};
use std::time::UNIX_EPOCH;

use fist_types::filters::SortOrder;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    run::{
        FsAction, FsPane,
        item::PathItem,
        state::{GLOBAL, STACK, TASKS},
    },
};

#[derive(Clone, Copy)]
pub struct SortMode {
    pub order: Option<SortOrder>,
    pub threshold: u32,
}

// order:    None    = no nucleo sort fn (db/rg panes, or the pane sort is none)
//           Some(_) = hard sort engaged (threshold u32::MAX)
static SORT_MODE: Mutex<SortMode> = Mutex::new(SortMode {
    order: None,
    threshold: 0,
});

static DIR_SIZE: LazyLock<fist_size::DirSizeCache> = LazyLock::new(fist_size::DirSizeCache::new);

pub fn set_sort(order: SortOrder) {
    SORT_MODE.lock().unwrap().order = (order != SortOrder::none).then_some(order);
}

pub fn get_sort() -> SortMode {
    *SORT_MODE.lock().unwrap()
}

/// The single place that pushes the current pane's sort into nucleo.
/// Never resorts: the resort belongs to the caller (`SetSort`).
pub fn update_sort(state: &mut MMState<'_, '_>) {
    // db/rg panes order by insertion order (SQL/rg already ordered)
    let order: Option<SortOrder> = STACK::with_current(|p| match p {
        FsPane::Files { .. }
        | FsPane::Folders { .. }
        | FsPane::Apps { .. }
        | FsPane::Search { .. } => None,
        p => (p.sort() != SortOrder::none).then_some(p.sort()),
    });
    let threshold = match order {
        None => STACK::with_current(FsPane::stability_threshold),
        Some(_) => u32::MAX,
    };
    // direct nucleo calls: matchmaker's Worker::set_stability auto-resorts
    *SORT_MODE.lock().unwrap() = SortMode { order, threshold };
    state.picker_ui.worker.nucleo.sort_with(sort_fn_for(order));
    state.picker_ui.worker.nucleo.set_stability(threshold);
}

pub type SortFn = Arc<dyn Fn((u32, &PathItem), (u32, &PathItem)) -> bool + Send + Sync>;
fn sort_fn_for(order: Option<SortOrder>) -> Option<SortFn> {
    match order? {
        // plain name, not render(): the sort fn may run off-thread
        SortOrder::name => Some(Arc::new(|(_i, a), (_j, b)| {
            a.display_name() < b.display_name()
        })),
        SortOrder::mtime | SortOrder::atime => {
            Some(Arc::new(|(_i, a), (_j, b)| a.value() < b.value()))
        }
        SortOrder::size => Some(Arc::new(|(_i, a), (_j, b)| {
            DIR_SIZE.get_path(&a.path).unwrap_or(0) < DIR_SIZE.get_path(&b.path).unwrap_or(0)
        })),

        // set_sort never stores none — defensive
        SortOrder::none => None,
    }
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

/// Populate-time metadata fill. Called inline before every push, gated on the
/// global sort mode — db/rg panes always have order None, so this only ever
/// stores for hard-sorted Nav/Find/Custom panes.
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
/// items, compute the new sort values off-thread, then dispatch `SetSort`.
pub fn fill_and_resort(
    state: &MMState<'_, '_>,
    order: SortOrder,
) {
    match order {
        // no fill needed: name compares paths, none unsorts
        SortOrder::name | SortOrder::none => {
            GLOBAL::send_action(FsAction::SetSort);
        }
        SortOrder::mtime | SortOrder::atime | SortOrder::size => {
            let items = state.picker_ui.worker.nucleo.items();
            TASKS::spawn_blocking(move || {
                match order {
                    SortOrder::mtime | SortOrder::atime => {
                        for (_, item) in items.iter() {
                            // if sort changed, return
                            item.set_value(stat_time(&item.path, order))
                        }
                    }
                    SortOrder::size => {
                        for (_, item) in items.iter() {
                            DIR_SIZE.add(item.path.inner());
                        }
                        DIR_SIZE.wait();
                    }
                    SortOrder::name | SortOrder::none => {}
                }

                GLOBAL::send_action(FsAction::SetSort);
            });
        }
    }
}

/// Post-populate size fill: paths were submitted during injection; wait for
/// the cache, then let `SetSort` apply the values and resort.
pub fn wait_sizes_then_resort() {
    TASKS::spawn_blocking(|| {
        DIR_SIZE.wait();
        GLOBAL::send_action(FsAction::SetSort);
    });
}
