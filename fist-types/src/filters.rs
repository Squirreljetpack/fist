use cli_boilerplate_automation::bath::PathExt;
use std::path::Path;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Default, strum_macros::Display, PartialEq, Eq, clap::ValueEnum)]
#[cfg_attr(
    feature = "serde",
    derive(
        serde::Serialize,
        serde::Deserialize,
        strum_macros::EnumIter,
        strum_macros::IntoStaticStr,
    )
)]
#[strum(serialize_all = "lowercase")]
pub enum SortOrder {
    name,
    mtime,
    #[default]
    none,
    /// Not always supported
    size,
}

impl SortOrder {
    pub fn cycle(&mut self) {
        *self = match self {
            SortOrder::mtime => SortOrder::none,
            SortOrder::none => SortOrder::name,
            SortOrder::name => SortOrder::mtime,
            SortOrder::size => SortOrder::mtime, // exclude size from loop
        };
    }
}

// ------------------------------------------------------------
#[derive(Debug, Default, Clone, clap::Args, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Visibility {
    /// show hidden files and folders
    #[arg(short = 'h')]
    pub hidden: bool,

    #[clap(skip)]
    /// show hidden files only.
    /// When combined with dir or files, the effect is inclusive: hidden or a file, hidden or a directory.
    pub hidden_only: bool,

    /// HIDE ignored files
    #[arg(short = 'I')]
    pub ignore: bool,

    /// show all
    #[arg(short = 'a', short_alias = 'u')]
    all: bool,

    /// only show directories
    #[arg(short = 'F')]
    pub dirs: bool,
    /// show only files
    #[arg(short = 'f')]
    pub files: bool,

    /// Don't follow symlinks (tui only).
    #[arg(skip)]
    pub no_follow: bool,
}

impl Visibility {
    pub const DEFAULT: Self = Self {
        all: false,
        no_follow: false,
        hidden: false,
        hidden_only: false,
        ignore: false,
        dirs: false,
        files: false,
    };

    // rust can probably optimize out unnecessary file accesses?
    pub fn filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;

        if self.hidden_only {
            push = if self.dirs {
                path.is_dir()
            } else if self.files {
                path.is_file()
            } else {
                path.is_hidden()
            }
        } else if !self.hidden {
            push = !path.is_hidden()
        }

        if self.ignore {
            // todo
        }

        if self.dirs {
            push = path.is_dir()
        } else if self.files {
            push = path.is_file()
        }

        if !self.all {
            push = path.exists()
        }
        push
    }

    pub fn post_fd_filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;

        if self.hidden_only {
            push = if self.dirs {
                path.is_dir()
            } else if self.files {
                path.is_file()
            } else {
                path.is_hidden()
            }
        };

        if !self.all {
            push = path.exists()
        }
        push
    }

    pub fn is_default(&self) -> bool {
        *self == Self::DEFAULT
    }

    pub fn all(&self) -> bool {
        self.all
    }
    pub fn set_all(
        &mut self,
        all: bool,
    ) {
        if all {
            *self = Visibility {
                all: true,
                hidden: false,
                dirs: self.dirs,
                no_follow: self.no_follow,
                ..Default::default()
            }
        } else {
            self.all = false;
        }
    }
    pub fn toggle_all(&mut self) {
        if self.all() {
            self.set_all(false)
        } else {
            self.set_all(true)
        }
    }
    pub fn set_default(&mut self) {
        *self = Visibility {
            dirs: self.dirs,
            ..Default::default()
        }
    }
    pub fn include_hidden(&self) -> bool {
        self.hidden || self.hidden_only
    }

    pub fn validated(mut self) -> Self {
        if self.all {
            self.set_all(true);
        }
        if self.dirs {
            self.files = false
        } else if self.files {
            self.dirs = false
        }
        self
    }
    // fn set_depth(&mut self, depth: usize) {
    //     self.depth = depth.max(1)
    // }
    // fn depth(&self) -> usize {
    //     self.depth
    // }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, Default, strum_macros::Display, PartialEq, clap::ValueEnum)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize, strum_macros::IntoStaticStr)
)]
#[strum(serialize_all = "lowercase")]
pub enum DbSortOrder {
    name,
    atime,
    /// Weighted frequency + recency
    #[default]
    frecency,
    count,
    none,
}

impl DbSortOrder {
    pub const fn cycle(&mut self) {
        *self = match self {
            Self::atime => Self::frecency,
            Self::frecency => Self::name,
            Self::name => Self::atime,
            _ => Self::atime, // exclude size from loop
        };
    }
}

// --------------------------------------------------
impl From<DbSortOrder> for SortOrder {
    fn from(v: DbSortOrder) -> Self {
        match v {
            DbSortOrder::name => SortOrder::name,
            DbSortOrder::atime => SortOrder::mtime,
            _ => SortOrder::none,
        }
    }
}

impl From<SortOrder> for DbSortOrder {
    fn from(v: SortOrder) -> Self {
        match v {
            SortOrder::name => DbSortOrder::name,
            SortOrder::mtime => DbSortOrder::atime,
            _ => DbSortOrder::frecency,
        }
    }
}
