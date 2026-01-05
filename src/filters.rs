use std::path::Path;

use cli_boilerplate_automation::bath::PathExt;
use strum::IntoStaticStr;

#[allow(non_camel_case_types)]
#[derive(
    Debug,
    clap::ValueEnum,
    Clone,
    Copy,
    Default,
    strum_macros::Display,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    strum_macros::EnumIter,
    strum_macros::IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum SortOrder {
    name,
    mtime,
    #[default]
    none,
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
#[derive(
    Debug, Default, Clone, Copy, clap::Args, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
pub struct Visibility {
    /// show hidden files and folders
    #[arg(short = 'h')]
    pub hidden: bool,

    #[arg(short = 'H')]
    /// show hidden files only
    pub hidden_files: bool,

    /// HIDE ignored files
    #[arg(short = 'I')]
    pub ignore: bool,

    /// show all
    #[arg(short = 'a', short_alias = 'u')]
    all: bool,

    /// only show directories
    #[arg(short = 'D')]
    pub dirs: bool,
    /// show only files, experimental
    #[arg(skip)]
    pub files: bool,
}

impl Visibility {
    pub const DEFAULT: Self = Self {
        all: false,
        hidden: false,
        hidden_files: false,
        ignore: false,
        dirs: false,
        files: false,
    };

    // can rust optimize out unnecessary file accesses?
    pub fn filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;
        if !self.hidden {
            push = !path.is_hidden()
        }
        if self.ignore {
            // todo
        }
        if self.hidden_files && path.is_hidden() {
            if self.dirs {
                push = path.is_dir()
            } else {
                push = !path.is_dir()
            }
        };
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
                dirs: self.dirs,
                hidden: false,
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

    pub fn validate(mut self) -> Self {
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
#[derive(
    Debug,
    clap::ValueEnum,
    Clone,
    Copy,
    Default,
    strum_macros::Display,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    IntoStaticStr,
)]
#[strum(serialize_all = "lowercase")]
pub enum DbSortOrder {
    name,
    atime,
    /// Frecency based
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
