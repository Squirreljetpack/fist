#![allow(clippy::upper_case_acronyms)]

use std::{cell::RefCell, sync::atomic::Ordering};

use log::{self};

use crate::{
    db::DbSortOrder,
    filters::{SortOrder, Visibility},
    run::{FsAction, FsPane, state::GLOBAL, state::STACK},
};

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
        match STACK::current() {
            FsPane::Nav { sort, vis, .. } => {
                FILTERS::with(|gsort, gvis| {
                    if gsort != &sort || gvis != &vis {
                        log::debug!("{sort} {vis:?} {gsort} {gvis:?}");
                        STACK::with_current_mut(|pane| {
                            if let FsPane::Nav { sort, vis, .. } = pane {
                                *sort = *gsort;
                                *vis = *gvis
                            }
                        });
                        // send the effect to trigger the formatter
                        GLOBAL::send_action(FsAction::Reload);
                    }
                });

                // The sync event isn't sent when matcher is never sent so this doesn't work
                // if complete.load(Ordering::Acquire) && state.picker_ui.results.status.item_count == 0 {
                //     log::debug!("Empty nav");
                //     if GLOBAL::with_cfg(|c| c.interface.toast_on_empty) {
                //         TOAST::push_msg(
                //             Span::styled("No results.", Style::new().dim().italic()),
                //             true,
                //         );
                //     }
                //     return efx![];
                // };
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
                    if gvis != &vis {
                        STACK::with_current_mut(|pane| {
                            if let FsPane::Fd { vis, .. } = pane {
                                *vis = *gvis;
                            } else if let FsPane::Custom { vis, .. } = pane {
                                *vis = *gvis;
                            }
                        });
                        // send the effect to trigger the formatter
                        GLOBAL::send_action(FsAction::Reload);
                    } else if gsort != &sort {
                        STACK::with_current_mut(|pane| {
                            if let FsPane::Fd { sort, .. } = pane {
                                // don't exchange
                                if complete.load(Ordering::Acquire) {
                                    *sort = *gsort;
                                    // todo
                                    // iterate over current items and repush
                                    // we can store in a global vec
                                }
                            } else if let FsPane::Custom { sort, .. } = pane {
                                if complete.load(Ordering::Acquire) {
                                    *sort = *gsort;
                                    // set items and reload
                                    GLOBAL::send_action(FsAction::Reload);
                                }
                            }
                        });
                    }
                });
            }
            FsPane::Files { sort, .. } | FsPane::Folders { sort, .. } => {
                FILTERS::with(|gsort, _| {
                    if DbSortOrder::from(*gsort) != sort {
                        log::debug!("changing sort: {gsort} -> {sort}");
                        STACK::with_current_mut(|pane| {
                            if let FsPane::Files { sort, .. } | FsPane::Folders { sort, .. } = pane
                            {
                                *sort = (*gsort).into();
                            }
                        });
                        // send the effect to trigger the formatter
                        GLOBAL::send_action(FsAction::Reload);
                    }
                });
            }

            _ => {}
        }
    }
}
