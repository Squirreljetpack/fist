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
pub struct ShouldNotAbortOnEmpty;

#[derive(Clone, Debug)]
pub struct InitialRelativePathSetting(pub bool);

#[derive(Debug)]
pub struct InitialPreserveWhitespaceInSearch;

/// Menu prompt configuration for the overlay input bar
#[derive(Debug, Clone)]
pub struct MenuPrompt {
    pub kind: PromptKind,
    pub title: String,
    pub initial: String,
    pub cursor: usize,
}

impl MenuPrompt {
    pub fn new(kind: PromptKind) -> Self {
        Self {
            title: kind.to_string(),
            kind,
            initial: String::new(),
            cursor: 0,
        }
    }

    pub fn title(
        mut self,
        value: impl Into<String>,
    ) -> Self {
        self.title = value.into();
        self
    }

    /// Set initial input value and move cursor to the end of it
    pub fn initial(
        mut self,
        value: impl Into<String>,
    ) -> Self {
        let s = value.into();
        self.cursor = s.len();
        self.initial = s;
        self
    }

    /// Set cursor position (grapheme index)
    pub fn cursor(
        mut self,
        pos: usize,
    ) -> Self {
        self.cursor = pos;
        self
    }
}

/// AbsPath: Previous Directory
/// u32: Stashed index
/// Visibility: Initial visibility if fd pane was initialized without pv, from --reset-visibility
pub struct STORE;

impl STORE {
    pub fn set<T: 'static + Debug>(value: T) {
        TLS_MAP.with(|map| {
            map.borrow_mut().insert::<T>(_dbg!("TlsSet"; value));
        });
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
    pub fn set_menu_prompt(menu_prompt: Option<MenuPrompt>) {
        if let Some(prompt) = menu_prompt {
            TLS_MAP.with(|map| {
                map.borrow_mut().insert(prompt);
            });
        }
    }

    pub fn set_menu_target(target: MenuTarget) {
        TLS_MAP.with(|map| {
            map.borrow_mut().insert(target);
        });
    }

    pub fn debug() {
        TLS_MAP.with(|map| {
            log::info!("TLS: {:#?}", map.borrow());
        });
    }
}
