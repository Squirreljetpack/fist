use crate::run::styles::FileStyles;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct StyleConfig {
    pub path: PathDisplayConfig,
}

// --------- Path Display -----------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PathDisplayConfig {
    /// Whether to shorten $HOME to ~
    pub collapse_home: bool,
    /// Whether to display paths relative to the current working directory
    pub relative: bool,
    /// Whether to display file icons to the left of entries
    pub file_icons: bool,
    /// Whether to color style files with colors
    pub file_colors: bool,
    /// Whether to display directory icons
    pub dir_icons: bool,
    /// Whether to color style directory with colors
    pub dir_colors: bool,
    /// Style configuration based on file type
    pub file_styles: FileStyles,

    // Experimental
    pub symlink: Option<bool>, // hide/normal/fancy(target/color?)
    pub invalid: Option<bool>, // hide/normal/fancy(color)
}

impl PathDisplayConfig {
    pub const DEFAULT: Self = Self {
        collapse_home: true,
        relative: true,
        file_icons: true,
        file_colors: true,
        dir_icons: true,
        dir_colors: true,
        symlink: None,
        invalid: None,
        file_styles: FileStyles::DEFAULT,
    };
}

impl Default for PathDisplayConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}
