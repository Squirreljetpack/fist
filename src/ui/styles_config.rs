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
    pub collapse_home: bool,
    pub relative: bool,
    pub file_icons: bool,
    pub file_colors: bool,
    pub dir_icons: bool,
    pub dir_colors: bool,
    pub symlink: Option<bool>, // hide/normal/fancy(target/color?)
    pub invalid: Option<bool>, // hide/normal/fancy(color)
    pub file_styles: FileStyles,
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
