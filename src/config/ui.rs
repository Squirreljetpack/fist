use cli_boilerplate_automation::define_const_default;
use matchmaker::config::HorizontalSeparator;

pub use super::styles::FileStyles;

define_const_default!(
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub struct StyleConfig {
        pub path: PathDisplayConfig,
        pub matchmaker: MatchmakerExtraConfig,
    }
);

// --------- Path Display -----------

define_const_default!(
    // #[partial(path)]
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub struct PathDisplayConfig {
        /// Whether to shorten $HOME to ~
        pub collapse_home: bool = true,
        /// Whether to display paths relative to the current working directory
        pub relative: bool = true,
        /// Whether to display file icons to the left of entries
        pub file_icons: bool = true,
        /// Whether to color files with colors
        pub file_colors: bool = true,
        /// Whether to display directory icons
        pub dir_icons: bool = true,
        /// Whether to color directory with colors
        pub dir_colors: bool = true,
        /// Style configuration based on file type
        pub file_styles: FileStyles,
        pub icon_colors: bool = true,

        // Experimental
        pub symlink: Option<bool> = None, // hide/normal/fancy(target/color?)
        pub invalid: Option<bool> = None, // hide/normal/fancy(color)
    }
);

// ------------------------------------------
define_const_default!(
    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    pub struct MatchmakerExtraConfig {
        /// Whether to shorten $HOME to ~
        pub horizontal_separator: HorizontalSeparator = HorizontalSeparator::Light,
    }
);
