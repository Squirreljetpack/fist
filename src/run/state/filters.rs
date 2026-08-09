#![allow(clippy::upper_case_acronyms)]

use std::cell::RefCell;

use fist_types::filters::Visibility;

thread_local! {
    static VIS: RefCell<Visibility> = const { RefCell::new(Visibility::DEFAULT) }
}

/// Global visibility filter. The pane is the source of truth for sort
/// (see [`super::sort`]); this only holds visibility, synced pane-ward by
/// [`crate::run::FsAction::Refilter`].
pub struct FILTERS {}

impl FILTERS {
    pub fn with_mut<T>(f: impl FnOnce(&mut Visibility) -> T) -> T {
        VIS.with(|cell| f(&mut cell.borrow_mut()))
    }

    pub fn with(f: impl FnOnce(&Visibility)) {
        VIS.with(|cell| f(&cell.borrow()));
    }

    // ------- convenience ------------
    pub fn visibility() -> Visibility {
        VIS.with(|cell| *cell.borrow())
    }

    pub fn set(vis: Visibility) {
        VIS.with(|cell| {
            *cell.borrow_mut() = vis;
        });
    }

    pub fn with_vis_mut<F: FnOnce(&mut Visibility)>(f: F) {
        VIS.with(|cell| {
            let mut borrow = cell.borrow_mut();
            f(&mut borrow);
        });
    }
}
