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
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Visibility {
    /// show hidden files and folders
    pub hidden: bool,
    /// show hidden files only.
    /// When combined with dir or files, the effect is inclusive: hidden or a file, hidden or a directory.
    pub hidden_only: bool,
    /// HIDE ignored files
    pub ignore: bool,
    /// show all
    all: bool,

    /// only show directories
    pub dirs: bool,
    /// show only files
    pub files: bool,

    /// Don't follow symlinks (tui only).
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

    // note: does rust know to get all these metadata checks in one go?
    pub fn post_nav_filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;

        if self.hidden_only {
            return path.is_hidden()
                || if self.dirs {
                    path.is_dir()
                } else if self.files {
                    path.is_file()
                } else {
                    false
                };
        } else if !self.hidden {
            push &= !path.is_hidden()
        }

        if self.dirs {
            push &= path.is_dir()
        } else if self.files {
            push &= path.is_file()
        }

        push
    }

    pub fn post_fd_filter(
        &self,
        path: &Path,
    ) -> bool {
        let mut push = true;

        if self.hidden_only {
            push = path.is_hidden()
                || if self.dirs {
                    path.is_dir()
                } else if self.files {
                    path.is_file()
                } else {
                    false
                };
        };

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

#[derive(Debug, Default, Clone, clap::Args, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct PartialVisibility {
    /// show hidden files and folders
    #[arg(
        short = 'h',
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
    )]
    pub hidden: Option<bool>,

    /// HIDE ignored files
    #[arg(
        short = 'I',
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
    )]
    pub ignore: Option<bool>,

    /// show all
    #[arg(
        short = 'a',
        short_alias = 'u',
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
    )]
    pub all: Option<bool>,

    /// only show directories
    #[arg(
        short = 'F',
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
    )]
    pub dirs: Option<bool>,

    /// show only files
    #[arg(
        short = 'f',
        num_args = 0..=1,
        default_missing_value = "true",
        value_parser = clap::value_parser!(bool),
    )]
    pub files: Option<bool>,

    /// Don't follow symlinks (tui only).
    #[arg(skip)]
    pub no_follow: Option<bool>,
}

impl PartialVisibility {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn into(self) -> Visibility {
        let mut vis = Visibility::default();
        vis.apply(self);
        vis
    }
}

impl Visibility {
    pub fn apply(
        &mut self,
        patch: PartialVisibility,
    ) {
        if let Some(v) = patch.hidden {
            self.hidden = v;
        }
        if let Some(v) = patch.ignore {
            self.ignore = v;
        }
        if let Some(v) = patch.all {
            self.all = v;
        }
        if let Some(v) = patch.dirs {
            self.dirs = v;
        }
        if let Some(v) = patch.files {
            self.files = v;
        }
        if let Some(v) = patch.no_follow {
            self.no_follow = v;
        }
        *self = self.validated();
    }

    pub fn from_cmd_or_cfg(
        cmd: PartialVisibility,
        cfg: PartialVisibility,
    ) -> Self {
        let mut vis = Self::default();

        if cmd.is_default() {
            vis.apply(cfg);
        } else {
            vis.apply(cmd);
        }

        vis
    }
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
