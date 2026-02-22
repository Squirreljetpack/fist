// -----------------------------------------------------------------------------
// Mocks to satisfy src/cli/types.rs dependencies
// -----------------------------------------------------------------------------
mod db {
    #[allow(non_camel_case_types)]
    #[derive(Debug, Copy, Clone, clap::ValueEnum)]
    pub enum DbTable {
        apps,
        files,
        dirs,
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
