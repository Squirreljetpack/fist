// -----------------------------------------------------------------------------
// Mocks to satisfy src/cli/types.rs dependencies
// -----------------------------------------------------------------------------

mod db {
    use clap::ValueEnum;

    #[allow(non_camel_case_types)]
    #[derive(Debug, Clone, ValueEnum, Default, strum_macros::Display)]
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

    #[derive(Debug, Copy, Clone, ValueEnum)]
    pub enum DbTable {
        #[value(name = "apps")]
        Apps,
        #[value(name = "files")]
        Files,
        #[value(name = "dirs")]
        Dirs,
    }
}

mod filters {
    #[derive(Debug, Clone, clap::ValueEnum, Default)]
    pub enum SortOrder {
        #[value(name = "name")]
        Name,
        #[value(name = "mtime")]
        Mtime,
        #[default]
        #[value(name = "none")]
        None,
        #[value(name = "size")]
        Size,
    }

    #[derive(Debug, Default, Clone, Copy, clap::Args, PartialEq, Eq)]
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

        /// show only files (tui only)
        #[arg(skip)]
        pub files: bool,
        /// Don't follow symlinks (tui only).
        #[arg(skip)]
        pub no_follow: bool,
    }
}

mod utils {
    include!("../src/utils/types.rs");
}

mod find {
    include!("../src/find/ft_arg.rs");
}

mod lessfilter {
    #[derive(Default, Debug, Clone, Copy, clap::ValueEnum, strum_macros::Display)]
    #[strum(serialize_all = "lowercase")]
    pub enum Preset {
        #[clap(alias = "p")]
        /// For the f:ist preview pane.
        ///
        /// see [`matchmaker::preview`]
        Preview,
        #[default]
        #[clap(alias = "d")]
        /// For terminal display.
        Display,
        #[clap(alias = "x")]
        /// For terminal interaction/verbose display.
        Extended,
        #[clap(alias = "i")]
        /// Metadata/raw info.
        Info,
        #[clap(alias = "o")]
        /// System open.
        ///
        /// (By deferring to fs :open)
        Open,
        /// Alternate (custom) open
        Alternate,
        #[clap(alias = "e")]
        // For [`crate::run::FsAction::Advance`]
        Edit,
    }
}
