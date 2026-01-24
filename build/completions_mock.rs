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

mod find {
    pub mod fd {
        #[derive(Debug, Clone)]
        pub enum FileTypeArg {
            Type(FileType),
            FileCategory(FileCategory),
            Ext(String),
            Group(String), // todo: custom groups in config
        }
        impl std::str::FromStr for FileTypeArg {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let s_lower = s.to_lowercase();

                // extension if starts with "."
                if let Some(s) = s_lower.strip_prefix('.') {
                    return Ok(FileTypeArg::Ext(s.to_string()));
                }

                // try parse as FileType
                if let Ok(ft) = FileType::from_str(&s_lower) {
                    return Ok(FileTypeArg::Type(ft));
                }

                // try parse as FileCategory
                if let Ok(cat) = FileCategory::from_str(&s_lower) {
                    return Ok(FileTypeArg::FileCategory(cat));
                }

                // fallback to group
                Ok(FileTypeArg::Group(s.to_string()))
            }
        }

        #[derive(Debug, Clone, PartialEq, Eq, std::hash::Hash)]
        pub enum FileCategory {
            Image,
            Video,
            Music,
            Lossless, // Lossless music, rather than any other kind of data...
            Crypto,
            Document,
            Compressed,
            Temp,
            Compiled,
            Build, // A “build file is something that can be run or activated somehow in order to
            // kick off the build of a project. It’s usually only present in directories full of
            // source code.
            Source,
            Configuration, // add configuration
            Text,
        }

        #[derive(
            Debug,
            strum_macros::Display,
            strum_macros::EnumString,
            Clone,
            Copy,
            PartialEq,
            Eq,
            std::hash::Hash,
        )]
        #[strum(serialize_all = "kebab-case")] // optional: converts variants to kebab-case by default
        pub enum FileType {
            #[strum(serialize = "f")]
            File,
            #[strum(serialize = "d")]
            Directory,
            #[strum(serialize = "l")]
            Symlink,
            #[strum(serialize = "b")]
            BlockDevice,
            #[strum(serialize = "c")]
            CharDevice,
            #[strum(serialize = "x")]
            Executable,
            #[strum(serialize = "e")]
            Empty,
            #[strum(serialize = "s")]
            Socket,
            #[strum(serialize = "p")]
            Pipe,
        }

        use std::str::FromStr;
        use thiserror::Error;

        #[derive(Debug, Error)]
        #[error("Invalid type: {0}")]
        pub struct ParseFileTypeError(pub String);

        impl FromStr for FileCategory {
            type Err = ParseFileTypeError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s.to_lowercase().as_str() {
                    "v" | "vid" | "video" => Ok(FileCategory::Video),
                    "i" | "img" | "image" => Ok(FileCategory::Image),
                    "a" | "aud" | "audio" => Ok(FileCategory::Music),
                    "l" | "lossless" => Ok(FileCategory::Lossless),
                    "crypto" => Ok(FileCategory::Crypto),
                    "doc" | "document" => Ok(FileCategory::Document),
                    "z" | "compressed" => Ok(FileCategory::Compressed),
                    "t" | "tmp" | "temp" => Ok(FileCategory::Temp),
                    // "b" | "build" => Ok(FileCategory::Build),
                    "s" | "src" | "source" | "code" => Ok(FileCategory::Source),
                    "o" | "compiled" => Ok(FileCategory::Compiled),
                    "conf" => Ok(FileCategory::Configuration),
                    "txt" => Ok(FileCategory::Text),
                    _ => Err(ParseFileTypeError(s.to_string())),
                }
            }
        }
    }
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
