#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case)]
use std::{cell::RefCell, fmt::Debug, sync::OnceLock};

use anymap::AnyMap;
use cba::{_dbg, define_collection_wrapper};

use crate::{
    abspath::AbsPath, cli::paths::lessfilter_cfg_path, config::pager_cfg,
    lessfilter::LessfilterConfig, ui::menu_overlay::PromptKind,
};

thread_local! {
    static TLS_MAP: RefCell<AnyMap> = RefCell::new(AnyMap::new());
}

/// The user's lessfilter config, loaded on first use so menu action
/// conditions can build [`FileData`](crate::lessfilter::file_rule::FileData).
pub static LESSFILTER_CFG: OnceLock<LessfilterConfig> = OnceLock::new();

pub fn lessfilter_cfg() -> &'static LessfilterConfig {
    LESSFILTER_CFG.get_or_init(|| {
        let cfg = std::fs::read_to_string(lessfilter_cfg_path()).ok();
        match cfg.as_deref().and_then(|s| toml::from_str(s).ok()) {
            Some(cfg) => cfg,
            None => {
                log::warn!(
                    "Failed to parse lessfilter config at {}; using defaults",
                    lessfilter_cfg_path().display()
                );
                LessfilterConfig::default()
            }
        }
    })
}

#[derive(Debug)]
pub struct ExecuteHandlerShouldProcessParent;

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

/// Set by the Apps pane populate when it spawns the recache task; ensures
/// recache isn't run more than once (thread-local, so only checked on the
/// populating thread).
#[derive(Debug, Clone)]
pub struct RanRecache;

define_collection_wrapper!(
    /// The targeted filepaths of the last accepted menu action, set on
    /// activation for the non-stash strategies and consumed by the execute
    /// handlers as the lua `paths` table — the paths the condition was
    /// evaluated against.
    #[derive(Debug, Clone)]
    MenuCommandPaths: Vec<AbsPath>
);

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

    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = value.into();
        self
    }

    /// Set initial input value and move cursor to the end of it
    pub fn initial(mut self, value: impl Into<String>) -> Self {
        let s = value.into();
        self.cursor = s.len();
        self.initial = s;
        self
    }

    /// Set cursor position (grapheme index)
    pub fn cursor(mut self, pos: usize) -> Self {
        self.cursor = pos;
        self
    }
}

/// AbsPath: Previous Directory
/// u32: Stashed index
/// Visibility: Initial visibility if fd pane was initialized without pv, from --reset-visibility
pub struct STORE;

/// Bat passthrough argument extras, handed to the pager via the STORE TLS map.
/// Consumed one-shot by `STORE::get_bat_opts`, output is wrapped with bat before paging iff this is present.
#[derive(Debug, Clone)]
pub struct BatOpts(pub Vec<String>);

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
    /// Set bat passthrough extras for the next [`STORE::get_bat_opts`] call.
    /// Called by the [`FsAction::Lessfilter`](crate::run::FsAction::Lessfilter)
    /// paging handler for the help (special=1) flow; consumed one-shot by
    /// `get_bat_opts` (take semantics).
    pub fn set_bat_opts(opts: Vec<String>) {
        STORE::set(BatOpts(opts));
    }

    /// Merge the store-held [`BatOpts`] extras (taken once) with the
    /// `pager.toml` `bat_opts` base ([`pager_cfg`]). Returns `None` when
    /// either is absent: no BatOpts was set for this paging (plain
    /// ExecutePaged → raw) or the config disables bat
    /// (`bat_opts = None`). Does not consult the environment.
    pub fn get_bat_opts() -> Option<Vec<String>> {
        let BatOpts(extra) = STORE::take::<BatOpts>()?;
        let mut opts = pager_cfg().bat_opts.clone()?;
        opts.extend(extra);
        Some(opts)
    }

    pub fn set_menu_prompt(menu_prompt: Option<MenuPrompt>) {
        if let Some(prompt) = menu_prompt {
            TLS_MAP.with(|map| {
                map.borrow_mut().insert(prompt);
            });
        }
    }

    pub fn debug() {
        TLS_MAP.with(|map| {
            log::info!("TLS: {:#?}", map.borrow());
        });
    }
}
