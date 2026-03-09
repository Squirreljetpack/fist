use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use cba::bring::split::join_with_single_quotes;
use matchmaker::preview::AppendOnly;

use crate::{
    abspath::AbsPath, cli::DefaultCommand, db::DbSortOrder, find::fd::auto_enable_hidden,
    run::item::PathItem,
};
use fist_types::{
    When,
    filetypes::FileTypeArg,
    filters::{PartialVisibility, SortOrder, Visibility},
};

#[derive(Debug, Clone)]
pub enum FsPane {
    Custom {
        cwd: AbsPath,
        stored: Option<AppendOnly<PathItem>>,
        cmd: (OsString, Vec<OsString>),
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        // experimental
        sort: SortOrder,
        vis: Visibility,
    },
    Stream {
        cwd: AbsPath,
        stored: Option<AppendOnly<PathItem>>,
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        // experimental
        sort: SortOrder,
        vis: Visibility,
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
        no_heading: bool,

        rg: Vec<OsString>,
        complete: Arc<AtomicBool>,
    },
    Files {
        sort: DbSortOrder,
        input: (String, u32), // input, INDEX
    },
    Folders {
        sort: DbSortOrder,
        input: (String, u32), // input, INDEX
    },
    Apps {
        sort: DbSortOrder,
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
        cmd: (OsString, Vec<OsString>),
        keep_store: bool,
    ) -> Self {
        Self::Custom {
            cwd,
            stored: keep_store.then(Default::default),
            cmd,
            vis: visibility,
            sort: SortOrder::none,
            complete: Default::default(),
            input: Default::default(),
        }
    }

    pub fn new_launch() -> Self {
        Self::Apps {
            sort: DbSortOrder::frecency,
        }
    }

    pub fn new_stream(
        cwd: AbsPath,
        visibility: Visibility,
        keep_store: bool,
    ) -> Self {
        Self::Stream {
            cwd,
            stored: keep_store.then(Default::default),
            vis: visibility,
            sort: SortOrder::none,
            complete: Default::default(),
            input: Default::default(),
        }
    }

    // default ignore: apply when not explicit in cli -- this can work on the partial
    // auto_enable hidden: when not explicit in cli
    pub fn new_fd_from_command(
        cmd: DefaultCommand,
        is_default_dir: bool,
        default_visibility: PartialVisibility,
        cwd: AbsPath,
    ) -> Self {
        let mut vis = Visibility::from_cmd_or_cfg(cmd.vis, default_visibility);
        if cmd.vis.hidden.is_none() && auto_enable_hidden(&cmd.paths) {
            vis.hidden = true;
        }

        let DefaultCommand {
            sort,
            types,
            paths,
            fd,
            ..
        } = cmd;

        Self::Find {
            cwd,
            complete: Default::default(),
            input: Default::default(),
            sort: sort.unwrap_or_default(),
            vis,
            types,
            paths,
            fd_args: fd,
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
        }
    }

    pub fn new_rg_full(
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
        //
        paths: Vec<PathBuf>,
        query: String,
        mut patterns: Vec<String>,
        filtering: bool,
        //
        context: [usize; 2],
        case: When,
        no_heading: bool,
        fixed_strings: bool,
        //
        rg: Vec<OsString>,
    ) -> Self {
        if patterns.is_empty() {
            patterns.push(String::new()); // rg requires at least one pattern
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
            no_heading,
            fixed_strings,

            rg,
            complete: Default::default(),
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
        sort: DbSortOrder,
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
        // matches!(self, FsPane::Nav { .. } | FsPane::Rg { .. })
        //     || matches!(self, |FsPane::Files { .. }| FsPane::Folders { .. }
        //         | FsPane::Launch { .. })
        //     || matches!(
        //         self,
        //         FsPane::Fd { .. } | FsPane::Custom { .. } | FsPane::Stream { .. }
        //     )
        true
    }

    #[inline]
    pub fn stability_threshold(&self) -> u32 {
        // 0 -> always sort
        match self {
            FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Apps { .. } => 5,
            FsPane::Search {
                filtering, sort, ..
            } => {
                if *filtering {
                    if matches!(sort, SortOrder::none) {
                        0
                    } else {
                        5
                    }
                } else {
                    u32::MAX
                }
            }
            FsPane::Custom { .. } | FsPane::Stream { .. } => 5, // maybe
            FsPane::Nav { sort, .. } | FsPane::Find { sort, .. } => {
                if matches!(sort, SortOrder::none) {
                    0
                } else {
                    5
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
            | FsPane::Stream { input, .. }
            | FsPane::Find { input, .. }
            | FsPane::Nav { input, .. }
            | FsPane::Files { input, .. }
            | FsPane::Folders { input, .. } => input.0.clone(),

            FsPane::Search {
                input,
                patterns,
                filtering,
                ..
            } => {
                if *filtering {
                    input.0.clone()
                } else {
                    join_with_single_quotes(patterns)
                }
            }
            _ => String::new(),
        }
    }

    pub fn vis_mut(&mut self) -> Option<&mut Visibility> {
        match self {
            FsPane::Custom { vis, .. }
            | FsPane::Stream { vis, .. }
            | FsPane::Find { vis, .. }
            | FsPane::Search { vis, .. }
            | FsPane::Nav { vis, .. } => Some(vis),

            FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Apps { .. } => None,
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
