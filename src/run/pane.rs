use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use matchmaker::preview::AppendOnly;

use crate::{
    abspath::AbsPath, cli::DefaultCommand, db::DbSortOrder, find::fd::auto_enable_hidden,
    run::item::PathItem,
};
use fist_types::{
    When,
    filetypes::FileTypeArg,
    filters::{SortOrder, Visibility},
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
    Fd {
        cwd: AbsPath,
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        sort: SortOrder,
        vis: Visibility,
        types: Vec<FileTypeArg>,
        paths: Vec<OsString>,
        fd_args: Vec<OsString>,
    },
    Rg {
        cwd: AbsPath,
        input: (String, u32), // input, INDEX
        filtering: bool,

        sort: SortOrder,
        vis: Visibility,

        paths: Vec<PathBuf>,
        context: [usize; 2],
        case: When,
        patterns: Vec<String>,

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
    Launch {
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
        Self::Launch {
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

    pub fn new_fd_from_command(
        mut cmd: DefaultCommand,
        cwd: AbsPath,
    ) -> Self {
        if auto_enable_hidden(&cmd.paths) {
            cmd.vis.hidden = true;
        }

        let DefaultCommand {
            sort,
            vis,
            types,
            paths,
            fd,
            ..
        } = cmd;

        Self::Fd {
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
        Self::Fd {
            paths: vec![cwd.inner().into()],
            cwd,
            complete: Default::default(),
            input: Default::default(),
            sort,
            vis: vis.validated(),
            types: Default::default(),
            fd_args: vec![],
        }
    }

    pub fn new_rg(
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
    ) -> Self {
        let context = Default::default();
        let case = Default::default();
        let paths = vec![cwd.inner()];

        Self::new_rg_full(
            cwd,
            sort,
            vis,
            paths,
            context,
            case,
            vec![String::new()],
            vec![],
        )
    }

    pub fn new_rg_full(
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
        //
        paths: Vec<PathBuf>,
        context: [usize; 2],
        case: When,
        patterns: Vec<String>,
        //
        rg: Vec<OsString>,
    ) -> Self {
        Self::Rg {
            cwd,
            input: Default::default(),
            filtering: false,

            sort,
            vis: vis.validated(),

            paths,
            context,
            case,
            patterns,

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

    #[inline]
    pub fn supports_sort(&self) -> bool {
        matches!(
            self,
            FsPane::Nav { .. } | FsPane::Custom { .. } | FsPane::Fd { .. }
        )
    }

    #[inline]
    pub fn stability_threshold(&self) -> u32 {
        // 0 -> always sort
        match self {
            FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Launch { .. } => 5,
            FsPane::Custom { .. } | FsPane::Stream { .. } => 5, // maybe
            _ => 0,
        }
    }

    #[inline]
    pub fn should_cancel_input_entering_dir(&self) -> bool {
        true
        // todo: allow customizing?
        // matches!(self, FsPane::Nav { .. } | FsPane::Launch { .. })
    }

    pub fn get_input(&self) -> String {
        match self {
            FsPane::Custom { input, .. }
            | FsPane::Stream { input, .. }
            | FsPane::Fd { input, .. }
            | FsPane::Rg { input, .. }
            | FsPane::Nav { input, .. }
            | FsPane::Files { input, .. }
            | FsPane::Folders { input, .. } => input.0.clone(),
            _ => String::new(),
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
