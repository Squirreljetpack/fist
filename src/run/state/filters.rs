#![allow(clippy::upper_case_acronyms)]

use std::{cell::RefCell, sync::atomic::Ordering};

use log::{self};

use crate::{
    db::DbSortOrder,
    run::{FsAction, FsPane, state::GLOBAL, state::STACK},
};
use fist_types::filters::{SortOrder, Visibility};

thread_local! {
    static SORT: RefCell<(SortOrder, Visibility)> = const { RefCell::new((SortOrder::none, Visibility::DEFAULT)) }
}

pub struct FILTERS {}

impl FILTERS {
    pub fn with_mut<T>(f: impl FnOnce(&mut SortOrder, &mut Visibility) -> T) -> T {
        SORT.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let (sort, vis) = &mut *borrow;
            f(sort, vis)
        })
    }

    pub fn with(f: impl FnOnce(&SortOrder, &Visibility)) {
        SORT.with(|cell| {
            let borrow = cell.borrow();
            let (sort, vis) = &*borrow;
            f(sort, vis);
        });
    }

    // ------- convenience ------------
    pub fn sort() -> SortOrder {
        SORT.with(|cell| cell.borrow().0)
    }
    pub fn visibility() -> Visibility {
        SORT.with(|cell| cell.borrow().1)
    }

    pub fn set(
        sort: SortOrder,
        vis: Visibility,
    ) {
        SORT.with(|cell| {
            *cell.borrow_mut() = (sort, vis);
        });
    }

    pub fn with_vis_mut<F: FnOnce(&mut Visibility)>(f: F) {
        SORT.with(|cell| {
            let mut borrow = cell.borrow_mut();
            f(&mut borrow.1); // borrow.1 is the Visibility
        });
    }

    // -----------------------------------
    /// Reload if pane filter differs from global filter
    pub fn refilter() {
        STACK::with_current_mut(|p| match p {
            FsPane::Nav { sort, vis, .. } | FsPane::Rg { sort, vis, .. } => {
                FILTERS::with(|gsort, gvis| {
                    if gsort != &*sort || gvis != &*vis {
                        log::debug!("updating filters: {sort} -> {gsort}, {vis:?} -> {gvis:?}");
                        *sort = *gsort;
                        *vis = *gvis;
                        // send the effect to trigger the formatter
                        GLOBAL::send_action(FsAction::Reload);
                    }
                });
            }
            FsPane::Custom {
                complete,
                sort,
                vis,
                ..
            }
            | FsPane::Fd {
                complete,
                sort,
                vis,
                ..
            } => {
                FILTERS::with(|gsort, gvis| {
                    log::debug!("complete: {complete:?}");
                    let mut reload = false;
                    if gvis != &*vis {
                        *vis = *gvis;
                        reload = true;
                    }
                    if gsort != &*sort {
                        *sort = *gsort; // todo: on completion, maybe fd can check if its different from initial? dunno
                        // also need check fd stores all elements properly
                        if complete.load(Ordering::Acquire) {
                            reload = true;
                        }
                    }
                    if reload {
                        log::debug!("updating filters: {sort} -> {gsort}, {vis:?} -> {gvis:?}");
                        // send the effect to trigger the formatter
                        GLOBAL::send_action(FsAction::Reload);
                    }
                });
            }
            FsPane::Files { sort, .. }
            | FsPane::Folders { sort, .. }
            | FsPane::Launch { sort, .. } => {
                FILTERS::with(|gsort, _| {
                    if DbSortOrder::from(*gsort) != *sort {
                        log::debug!("updating filters: {gsort} -> {sort}");
                        *sort = (*gsort).into();
                        GLOBAL::send_action(FsAction::Reload);
                    }
                });
            }

            _ => {}
        })
    }
}
