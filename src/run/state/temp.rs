#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case)]
use std::{cell::RefCell, fmt::Debug};

use anymap::AnyMap;
use cba::_dbg;

use crate::{
    abspath::AbsPath,
    ui::menu_overlay::{MenuTarget, PromptKind},
};

thread_local! {
    static TLS_MAP: RefCell<AnyMap> = RefCell::new(AnyMap::new());
}

#[derive(Debug)]
pub struct ExecuteHandlerShouldProcessParent;

/// Snapshot of the picker state captured when the menu opens. Menu action
/// conditions are evaluated once against it (see
/// [`crate::spawn::menu_action`]), so the targets are frozen while the menu
/// is open.
#[derive(Debug, Clone, Default)]
pub struct MenuContext {
    /// Selected paths in selection order.
    pub selected: Vec<AbsPath>,
    /// The cursor item path; `None` when the cursor is disabled or has no item.
    pub cursor: Option<AbsPath>,
    /// Whether the query prompt was active when the menu opened.
    pub in_prompt: bool,
    /// The current directory (required by prompt-scoped conditions).
    pub cwd: Option<AbsPath>,
}

/// Set by [`crate::run::ahandlers::fs_reload`] when a new pane's sort
/// override (panesetting `default_sort`) is applied; consumed by
/// [`crate::run::FsAction::ReSort`]. While present, the column-2 metadata
/// override (the sort-value display) is skipped (see [`crate::run::start::format_tail`]).
#[derive(Debug, Clone)]
pub struct HideMetadata;

#[derive(Debug, Clone)]
pub struct ShouldNotAbortOnEmpty;

/// Set by [`crate::cli::handlers`] (cd-mode dirs/custom panes): while it is set,
/// populating at stack index 0 renders without a cwd (`RENDER_PATH = None`),
/// so `path.relative` never triggers on the initial pane.
#[derive(Debug, Clone)]
pub struct InitialNoRelative;

#[derive(Debug)]
pub struct InitialPreserveWhitespaceInSearch;

/// Prompt-mode flag (raw): the query bar is active (border shown, left/right
/// edit the query, Accept is intercepted). Set on entry by
/// [`crate::run::ahandlers::lock_prompt`] — gated on
/// `interface.prompt_locking`, so with locking off only the cwd lock
/// ([`crate::run::ahandlers::enter_prompt`]) sets it — and taken on leave
/// (never gated).
///
/// Invariant: cursor_disabled ⇒ cwd exists — the cursor is only disabled by
/// [`crate::run::ahandlers::enter_prompt`], which requires a cwd.
#[derive(Debug, Clone)]
pub struct InPrompt;

/// Set by the action aliaser when an accept keypress resolves to the *print*
/// flavor (`alt_accept` XOR alt-enter, outside app panes); consumed by the
/// [`matchmaker::Matchmaker`] accept hook, which then emits the selection and
/// returns nothing for the opener/apps to consume.
#[derive(Debug)]
pub struct AcceptFlavor;

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
            map.borrow_mut().insert::<T>(_dbg!("TlsSet", value));
        });
    }

    pub fn get<T: Clone + 'static>() -> Option<T> {
        TLS_MAP.with(|map| map.borrow().get::<T>().cloned())
    }

    pub fn contains<T: Clone + 'static>() -> bool {
        TLS_MAP.with(|map| map.borrow().get::<T>().is_some())
    }

    pub fn take<T: 'static + Debug>() -> Option<T> {
        _dbg!(
            "TlsTake",
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

    pub fn set_menu_context(context: MenuContext) {
        TLS_MAP.with(|map| {
            map.borrow_mut().insert(context);
        });
    }

    pub fn debug() {
        TLS_MAP.with(|map| {
            log::info!("TLS: {:#?}", map.borrow());
        });
    }
}
