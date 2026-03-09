#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case)]
use std::{cell::RefCell, fmt::Debug};

use anymap::AnyMap;
use cba::_dbg;

use crate::ui::menu_overlay::{MenuTarget, PromptKind};

thread_local! {
    static TLS_MAP: RefCell<AnyMap> = RefCell::new(AnyMap::new());
}

#[derive(Debug)]
pub struct ExecuteHandlerShouldProcessParent;

#[derive(Debug, Clone)]
pub struct InitialQueryShouldNotAbort;

#[derive(Clone, Debug)]
pub struct InitialRelativePathSetting(pub bool);

/// Option<AbsPath>: Previous Directory
/// Option<u32>: Stashed index
pub struct TlsStore;

impl TlsStore {
    pub fn set<T: 'static + Debug>(value: T) {
        TLS_MAP.with(|map| {
            map.borrow_mut().insert::<T>(_dbg!("TlsSet"; value));
        });
    }
    pub fn maybe_set<T: 'static + Debug>(value: Option<T>) {
        if let Some(v) = value {
            TlsStore::set(v);
        }
    }

    pub fn get<T: Clone + 'static>() -> Option<T> {
        TLS_MAP.with(|map| map.borrow().get::<T>().cloned())
    }

    pub fn take<T: 'static + Debug>() -> Option<T> {
        _dbg!(
            "TlsTake";
            TLS_MAP.with(|map| map.borrow_mut().remove::<T>())
        )
    }

    pub fn with<T: 'static, R>(f: impl FnOnce(&T) -> R) -> Option<R> {
        TLS_MAP.with(|map| {
            let borrow = map.borrow();
            borrow.get::<T>().map(f)
        })
    }

    pub fn with_mut<T: 'static, R>(f: impl FnOnce(&mut T) -> R) -> Option<R> {
        TLS_MAP.with(|map| {
            let mut borrow = map.borrow_mut();
            borrow.get_mut::<T>().map(f)
        })
    }

    /// If menu_prompt is set, menu starts an input overlay.
    ///
    /// The Ok variant of menu_target describes the target,
    /// while the Err variant corresponds to no target
    /// -- instead defining the cwd context, in which case
    /// only a restrictred subset of the menu actions is available.
    ///
    /// # Additional
    /// When the prompt is set and the target is Ok, the target's filename is shown in the title of the input bar.
    pub fn set_input_bar(
        menu_prompt: Option<PromptKind>,
        menu_target: MenuTarget,
    ) {
        TLS_MAP.with(|map| {
            let mut map = map.borrow_mut();
            if let Some(menu_prompt) = menu_prompt {
                map.insert(menu_prompt);
            }
            map.insert(menu_target);
        });
    }

    pub fn debug() {
        TLS_MAP.with(|map| {
            log::info!("TLS: {:#?}", map.borrow());
        });
    }
}
