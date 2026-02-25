mod execute;
mod status;
pub use status::*;

use std::{cell::RefCell, ffi::OsString, sync::Mutex};

use cli_boilerplate_automation::bath::{PathExt, auto_dest_for_src};

use crate::{
    abspath::AbsPath,
    cli::paths::__home,
    run::state::{GLOBAL, TASKS},
};

#[derive(Debug)]
pub struct SimpleStack<T = StashItem> {
    stack: Vec<T>, // not indexmap because need const
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum StashAction {
    Copy,
    Move,
    Custom,
}

#[derive(Debug, Clone)]
pub struct StashItem {
    pub kind: StashAction,
    pub src: AbsPath,
    pub status: StashItemStatus,
    pub dst: OsString,
}

impl StashItem {
    pub fn cp(path: AbsPath) -> Self {
        Self {
            kind: StashAction::Copy,
            status: StashItemStatus::new(&path),
            src: path,
            dst: Default::default(),
        }
    }

    pub fn mv(path: AbsPath) -> Self {
        Self {
            kind: StashAction::Move,
            status: StashItemStatus::new(&path),
            src: path,
            dst: Default::default(),
        }
    }

    pub fn custom(path: AbsPath) -> Self {
        Self {
            kind: StashAction::Custom,
            status: StashItemStatus::new(&path),
            src: path,
            dst: Default::default(),
        }
    }

    pub fn display(&self) -> String {
        self.src.display_short(__home())
    }

    pub fn is_custom(&self) -> bool {
        matches!(self.kind, StashAction::Custom)
    }
}

// -------- GLOBAL ---------

/// The state can be toggled in the overlay (TODO)
#[derive(Debug, Default, Clone, Copy, strum_macros::Display)]
pub enum CustomStashActionActionState {
    #[default]
    Symlink,
    Other,
    // This is different from the other states in being exclusive. When in this state:
    // - non-app actions (including custom-type) are not processed (transferred/cleared/etc.)
    // - non-app actions are not displayed.
    App,
}

// -------- GLOBAL ---------
pub type AlternateStashItem = AbsPath;

thread_local! {
    static MAIN_STASH: RefCell<(SimpleStack, CustomStashActionActionState)> = const { RefCell::new((SimpleStack::new(), CustomStashActionActionState::Symlink)) };
    // note: we don't necessarily just want a path here
    // note: we could support more exclusive stashes variants above which would also be stored here, which are also mututally exclusive
    static ALTERNATE_STASH: RefCell<SimpleStack<AlternateStashItem>> = const { RefCell::new(SimpleStack::new()) };
}
pub static STASH_ACTION_HISTORY: Mutex<Vec<StashItem>> = const { Mutex::new(Vec::new()) };

pub struct STASH;

impl STASH {
    pub fn extend(items: impl IntoIterator<Item = StashItem>) {
        for item in items {
            MAIN_STASH.with_borrow_mut(|s| insert_once(&mut s.0.stack, item, false));
        }
    }

    pub fn push_custom(path: AbsPath) {
        if matches!(STASH::cas(), CustomStashActionActionState::App) {
            ALTERNATE_STASH.with_borrow_mut(|s| insert_once(&mut s.stack, path, false))
        } else {
            MAIN_STASH.with_borrow_mut(|s| {
                let item = StashItem::custom(path);
                insert_once(&mut s.0.stack, item, false)
            });
        }
    }

    pub fn accept(index: usize) {
        if matches!(STASH::cas(), CustomStashActionActionState::App) {
            // todo
        } else {
            MAIN_STASH.with_borrow(|s| {
                let mut item = s.0.stack[index].clone();
                let custom_action = s.1;

                item.dst = GLOBAL::with_cfg(|c| {
                    auto_dest_for_src(&item.src, &item.dst, &c.fs.rename_policy)
                })
                .into();
                TASKS::spawn_blocking(move || item.transfer(custom_action));
            });
        }
    }

    pub fn remove(index: usize) {
        if matches!(STASH::cas(), CustomStashActionActionState::App) {
            ALTERNATE_STASH.with_borrow_mut(|s| s.stack.remove(index));
        } else {
            MAIN_STASH.with_borrow_mut(|s| s.0.stack.remove(index));
        }
    }

    pub fn with<R>(f: impl FnOnce(&SimpleStack) -> R) -> R {
        MAIN_STASH.with(|cell| f(&cell.borrow().0))
    }

    pub fn with_mut<R>(
        f: impl FnOnce((&mut SimpleStack, &mut CustomStashActionActionState)) -> R
    ) -> R {
        MAIN_STASH.with(|cell| {
            let mut borrow = cell.borrow_mut();
            let (stack, state) = &mut *borrow;
            f((stack, state))
        })
    }

    pub fn with_alternate<R>(f: impl FnOnce(&SimpleStack<AlternateStashItem>) -> R) -> R {
        ALTERNATE_STASH.with(|cell| f(&cell.borrow()))
    }

    pub fn with_alternate_mut<R>(f: impl FnOnce(&mut SimpleStack<AlternateStashItem>) -> R) -> R {
        ALTERNATE_STASH.with(|cell| f(&mut cell.borrow_mut()))
    }

    pub fn cas() -> CustomStashActionActionState {
        MAIN_STASH.with(|cell| {
            let borrow = cell.borrow();
            let (_, state) = &*borrow;
            *state
        })
    }

    pub fn set_cas(state: CustomStashActionActionState) {
        MAIN_STASH.with(|cell| {
            cell.borrow_mut().1 = state;
        });
    }

    pub fn stashed_apps() -> Vec<OsString> {
        todo!()
    }
}

// --------------------------------------------------------------

impl<T> SimpleStack<T> {
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl<T> std::ops::Deref for SimpleStack<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.stack
    }
}

// helpers

// pub fn toggle_insert<T: PartialEq>(
//     list: &mut Vec<T>,
//     item: T,
// ) {
//     if let Some(i) = list.iter().position(|x| *x == item) {
//         list.remove(i);
//     } else {
//         list.push(item);
//     }
// }

pub fn insert_once<T: PartialEq>(
    list: &mut Vec<T>,
    item: T,
    stable: bool,
) {
    if stable {
        if !list.contains(&item) {
            list.push(item);
        }
    } else {
        if let Some(i) = list.iter().position(|x| *x == item) {
            list.remove(i);
        }
        list.push(item);
    }
}

impl PartialEq for StashItem {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.src == other.src
    }
}

impl Eq for StashItem {}
