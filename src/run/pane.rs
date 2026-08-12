use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use cba::bring::split::join_with_single_quotes;
use matchmaker::preview::AppendOnly;

use crate::{
    abspath::AbsPath,
    run::{
        item::PathItem,
        lua::{LuaFn, compile_script},
        state::{GLOBAL, InitialPreserveWhitespaceInSearch, STORE},
    },
};
use fist_types::{
    When,
    filetypes::FileTypeArg,
    filters::{SortOrder, Visibility},
};

/// PartialEq is defined by discriminant
#[derive(Debug)]
pub enum FsPane {
    Custom {
        cwd: AbsPath,
        stored: Option<AppendOnly<PathItem>>,
        /// Some = spawn and read its stdout; None = read stdin
        cmd: Option<(OsString, Vec<OsString>)>,
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        // experimental
        sort: SortOrder,
        vis: Visibility,

        transform: Option<LuaFn>,
        tail_sep: Option<char>,
        input_sep: Option<char>,
    },
    Find {
        cwd: AbsPath,
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        sort: SortOrder,
        vis: Visibility,
        types: Vec<FileTypeArg>,
        paths: Vec<OsString>,
        fd_args: Vec<OsString>,
        transform: Option<LuaFn>,
    },
    Search {
        cwd: AbsPath,
        input: (String, u32), // input, INDEX
        filtering: bool,

        sort: SortOrder,
        vis: Visibility,

        paths: Vec<PathBuf>,
        context: [usize; 2],
        case: When,
        patterns: Vec<String>,
        fixed_strings: bool,
        one_line: bool,

        rg: Vec<OsString>,
        complete: Arc<AtomicBool>,
        is_initial: std::cell::RefCell<bool>,
    },
    Files {
        sort: SortOrder,
        input: (String, u32), // input, INDEX
    },
    Folders {
        sort: SortOrder,
        input: (String, u32), // input, INDEX
    },
    Apps {
        sort: SortOrder,
    },
    /// Listing of a named stash from the `stashes` db table.
    /// No visibility: entries are explicit additions, not directory contents.
    Stash {
        stash_name: String,
        sort: SortOrder,
        input: (String, u32), // input, INDEX
    },
    Nav {
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
        input: (String, u32), // input, INDEX
        complete: Arc<AtomicBool>,
        depth: usize,
    },
}

impl FsPane {
    /// Converts cwd to normalized absolute and stores it
    /// Executes cmd, otherwise populates from stdin
    pub fn new_custom(
        cwd: AbsPath,
        visibility: Visibility,
        cmd: Option<(OsString, Vec<OsString>)>,
        keep_store: bool,
        sort: SortOrder,
        transform: Option<String>,
        tail_sep: Option<char>,
        input_sep: Option<char>,
    ) -> Self {
        Self::Custom {
            cwd,
            stored: keep_store.then(Default::default),
            cmd,
            vis: visibility,
            sort,
            complete: Default::default(),
            input: Default::default(),
            transform: compile_script("--transform", transform),
            tail_sep,
            input_sep,
        }
    }

    pub fn new_launch() -> Self {
        Self::Apps {
            sort: SortOrder::none,
        }
    }

    pub fn new_stash(
        stash_name: String,
        sort: SortOrder,
    ) -> Self {
        Self::Stash {
            stash_name,
            sort,
            input: (String::new(), 0),
        }
    }

    pub fn new_fd_full(
        cwd: AbsPath,
        vis: Visibility,
        sort: Option<SortOrder>,
        types: Vec<FileTypeArg>,
        paths: Vec<OsString>,
        fd: Vec<OsString>,
        transform: Option<String>,
    ) -> Self {
        Self::Find {
            cwd,
            complete: Default::default(),
            input: Default::default(),
            sort: sort.unwrap_or_default(),
            vis,
            types,
            paths,
            fd_args: fd,
            transform: compile_script("--transform", transform),
        }
    }

    /// Create a fd pane in the current directory
    pub fn new_fd(
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
    ) -> Self {
        Self::Find {
            paths: vec![cwd.inner().into(), ".".into()], // last is pattern
            cwd,
            complete: Default::default(),
            input: Default::default(),
            sort,
            vis: vis.validated(),
            types: Default::default(),
            fd_args: vec![],
            transform: None,
        }
    }

    pub fn new_rg(
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
        //
        mut paths: Vec<PathBuf>,
        query: String,
        patterns: Vec<String>,
        filtering: bool,
        //
        context: [usize; 2],
        case: When,
        one_line: bool,
        fixed_strings: bool,
        //
        rg: Vec<OsString>,
    ) -> Self {
        if paths.is_empty() {
            paths.push(cwd.inner());
        }
        Self::Search {
            cwd,
            input: (query, 0),
            filtering,

            sort,
            vis: vis.validated(),

            paths,
            context,
            case,
            patterns,
            one_line,
            fixed_strings,

            rg,
            complete: Default::default(),
            is_initial: true.into(),
        }
    }

    pub fn new_nav(
        cwd: AbsPath,
        vis: Visibility,
        sort: SortOrder,
    ) -> Self {
        Self::Nav {
            cwd,
            sort,
            vis: vis.validated(),
            depth: 1,
            input: Default::default(),
            complete: Default::default(),
        }
    }

    pub fn new_history(
        folders: bool,
        sort: SortOrder,
    ) -> Self {
        if folders {
            Self::Folders {
                sort,
                input: (String::new(), 0),
            }
        } else {
            Self::Files {
                sort,
                input: (String::new(), 0),
            }
        }
    }
}

// ------ Utilities
impl FsPane {
    #[inline]
    pub fn sort(&self) -> SortOrder {
        match self {
            FsPane::Custom { sort, .. }
            | FsPane::Find { sort, .. }
            | FsPane::Search { sort, .. }
            | FsPane::Files { sort, .. }
            | FsPane::Folders { sort, .. }
            | FsPane::Apps { sort }
            | FsPane::Stash { sort, .. }
            | FsPane::Nav { sort, .. } => *sort,
        }
    }

    #[inline]
    pub fn sort_mut(&mut self) -> &mut SortOrder {
        match self {
            FsPane::Custom { sort, .. }
            | FsPane::Find { sort, .. }
            | FsPane::Search { sort, .. }
            | FsPane::Files { sort, .. }
            | FsPane::Folders { sort, .. }
            | FsPane::Apps { sort }
            | FsPane::Stash { sort, .. }
            | FsPane::Nav { sort, .. } => sort,
        }
    }

    #[inline]
    pub fn vis(&self) -> Option<Visibility> {
        match self {
            FsPane::Custom { vis, .. }
            | FsPane::Find { vis, .. }
            | FsPane::Search { vis, .. }
            | FsPane::Nav { vis, .. } => Some(*vis),

            FsPane::Files { .. }
            | FsPane::Folders { .. }
            | FsPane::Apps { .. }
            | FsPane::Stash { .. } => None,
        }
    }

    #[inline]
    pub fn supports_vis(&self) -> bool {
        matches!(
            self,
            FsPane::Nav { .. }
                | FsPane::Custom { .. }
                | FsPane::Find { .. }
                | FsPane::Search { .. }
        )
    }

    #[inline]
    pub fn supports_sort(&self) -> bool {
        !self.sort_options().is_empty()
    }

    /// The sort orders this pane type can engage — the single source of
    /// truth for the overlay list, the `n/m/a/s` keys, CLI/config validation,
    /// and [`Self::supports_sort`].
    ///
    /// - Search panes delegate to rg's own flags (`--sort`/`--sortr`), which
    ///   have no size sort.
    /// - db panes order via SQL; `size` means entry count, `none` frecency,
    ///   and mtime has no SQL arm (see [`SortOrder::label`]).
    pub fn sort_options(&self) -> &'static [SortOrder] {
        match self {
            FsPane::Search { .. } => Self::search_sort_options(),
            FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Apps { .. } => &[
                SortOrder::name,
                SortOrder::atime,
                SortOrder::size,
                SortOrder::none,
            ],
            // db pane: `atime` orders by add time; no size/frecency arms
            FsPane::Stash { .. } => &[SortOrder::name, SortOrder::atime, SortOrder::none],
            FsPane::Nav { .. } | FsPane::Find { .. } | FsPane::Custom { .. } => &[
                SortOrder::name,
                SortOrder::mtime,
                SortOrder::atime,
                SortOrder::size,
                SortOrder::none,
            ],
        }
    }

    /// Sort options of [`FsPane::Search`] — usable without a pane instance
    /// (CLI/config validation chokepoint).
    pub fn search_sort_options() -> &'static [SortOrder] {
        &[
            SortOrder::name,
            SortOrder::mtime,
            SortOrder::atime,
            SortOrder::none,
        ]
    }

    /// Whether populate has finished injecting items — used by the selection
    /// refill to wait out async populates. db panes populate in one batch and
    /// don't track completion.
    #[inline]
    pub fn is_complete(&self) -> bool {
        match self {
            FsPane::Custom { complete, .. }
            | FsPane::Find { complete, .. }
            | FsPane::Search { complete, .. }
            | FsPane::Nav { complete, .. } => complete.load(std::sync::atomic::Ordering::Acquire),
            FsPane::Files { .. }
            | FsPane::Folders { .. }
            | FsPane::Apps { .. }
            | FsPane::Stash { .. } => true,
        }
    }

    #[inline]
    pub fn stability_threshold(&self) -> u32 {
        // 0 -> always sort
        match self {
            FsPane::Files { .. }
            | FsPane::Folders { .. }
            | FsPane::Apps { .. }
            | FsPane::Stash { .. } => 5,
            FsPane::Search {
                filtering, sort, ..
            } => {
                if *filtering {
                    if matches!(sort, SortOrder::none) {
                        0
                    } else {
                        40
                    }
                } else {
                    u32::MAX
                }
            }
            FsPane::Custom { .. } => 40, // maybe
            FsPane::Nav { sort, .. } | FsPane::Find { sort, .. } => {
                if matches!(sort, SortOrder::none) {
                    0
                } else {
                    40
                }
            }
        }
    }

    #[inline]
    pub fn should_cancel_input_entering_dir(&self) -> bool {
        true
        // todo: lowpri: allow customizing?
    }

    /// initialize input on new pane, see [`crate::run::ahandlers::fs_post_reload_new`]
    pub fn get_input(&self) -> String {
        match self {
            FsPane::Custom { input, .. }
            | FsPane::Find { input, .. }
            | FsPane::Nav { input, .. }
            | FsPane::Files { input, .. }
            | FsPane::Folders { input, .. }
            | FsPane::Stash { input, .. } => input.0.clone(),

            FsPane::Search {
                input,
                patterns,
                filtering,
                ..
            } => {
                if *filtering {
                    input.0.clone()
                } else {
                    let mut s = join_with_single_quotes(patterns);
                    if GLOBAL::with_cfg(|c| c.panes.search.preserve_whitespace)
                        || STORE::take::<InitialPreserveWhitespaceInSearch>().is_some()
                    {
                        if !s.starts_with('\'') && !s.chars().any(|c| c.is_whitespace()) {
                            s.insert(0, '\'');
                        }
                    };
                    s
                }
            }
            _ => String::new(),
        }
    }

    pub fn vis_mut(&mut self) -> Option<&mut Visibility> {
        match self {
            FsPane::Custom { vis, .. }
            | FsPane::Find { vis, .. }
            | FsPane::Search { vis, .. }
            | FsPane::Nav { vis, .. } => Some(vis),

            FsPane::Files { .. }
            | FsPane::Folders { .. }
            | FsPane::Apps { .. }
            | FsPane::Stash { .. } => None,
        }
    }
}

// --------------------BOILERPLATE-------------------------------

impl PartialEq for FsPane {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}
impl Eq for FsPane {}
