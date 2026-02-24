use std::path::PathBuf;

use crate::run::FsPane;
use fist_types::filters::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanesSettings {
    pub display_script_simultaneous_count: usize,
    pub display_script_batch_size: usize,
}

impl Default for PanesSettings {
    fn default() -> Self {
        Self {
            display_script_simultaneous_count: 15,
            display_script_batch_size: 1000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Pane-specific settings
pub struct PanesConfig {
    pub app: AppPaneSettings,
    pub history: HistoryPaneSettings,
    pub nav: NavPaneSettings,
    pub stream: PaneSettings,
    pub fd: FdPaneSettings,
    pub rg: RgPaneSettings,
    pub custom: PaneSettings,

    pub settings: PanesSettings,
}

// enter prompt by default because it is less surprising
impl Default for PanesConfig {
    fn default() -> Self {
        Self {
            app: AppPaneSettings {
                ..Default::default()
            },
            history: HistoryPaneSettings {
                ..Default::default()
            },
            nav: NavPaneSettings::default(),
            fd: FdPaneSettings {
                ..Default::default()
            },
            rg: RgPaneSettings {
                ..Default::default()
            },
            custom: PaneSettings {
                ..Default::default()
            },
            stream: PaneSettings {
                ..Default::default()
            },

            settings: PanesSettings::default(),
        }
    }
}

impl PanesConfig {
    pub fn prompt(
        &self,
        pane: &FsPane,
    ) -> Option<String> {
        match pane {
            FsPane::Custom { .. } => self.custom.prompt.clone(),
            FsPane::Stream { .. } => self.stream.prompt.clone(),
            FsPane::Fd { .. } => self.fd.prompt.clone(),
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.prompt.clone(),
            FsPane::Launch { .. } => self.app.prompt.clone(),
            FsPane::Nav { .. } => self.nav.prompt.clone(),
            FsPane::Rg { .. } => self.rg.prompt.clone(),
        }
    }

    pub fn enter_prompt(
        &self,
        pane: &FsPane,
    ) -> bool {
        match pane {
            FsPane::Custom { .. } => self.custom.enter_prompt,
            FsPane::Stream { .. } => self.stream.enter_prompt,
            FsPane::Fd { .. } => self.fd.enter_prompt,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.enter_prompt,
            FsPane::Launch { .. } => self.app.enter_prompt,
            FsPane::Nav { .. } => false,
            FsPane::Rg { .. } => self.rg.enter_prompt,
        }
    }

    pub fn preview_show(
        &self,
        pane: &FsPane,
    ) -> Option<bool> {
        match pane {
            FsPane::Custom { .. } => self.custom.show_preview,
            FsPane::Stream { .. } => self.stream.show_preview,
            FsPane::Fd { .. } => self.fd.show_preview,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.show_preview,
            FsPane::Launch { .. } => self.app.show_preview,
            FsPane::Nav { .. } => self.nav.show_preview,
            FsPane::Rg { .. } => self.rg.show_preview,
        }
    }

    pub fn preview_layout_index(
        &self,
        pane: &FsPane,
    ) -> u8 {
        match pane {
            FsPane::Custom { .. } => self.custom.preview_layout_index,
            FsPane::Stream { .. } => self.stream.preview_layout_index,
            FsPane::Fd { .. } => self.fd.preview_layout_index,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.preview_layout_index,
            FsPane::Launch { .. } => self.app.preview_layout_index,
            FsPane::Nav { .. } => self.nav.preview_layout_index,
            FsPane::Rg { .. } => self.rg.preview_layout_index,
        }
    }
}
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: bool,

    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
}
impl Default for PaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            enter_prompt: true,
            preview_layout_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: bool,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// Default visibility when no visibility is specified.
    pub default_visibility: Visibility,
    /// When leaving the fd pane, untoggle the `only show directories` visibility filter.
    pub on_leave_unset_dirs_only: bool,
}

impl Default for FdPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            enter_prompt: true,
            preview_layout_index: 0,

            default_visibility: Default::default(),
            on_leave_unset_dirs_only: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RgPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: bool,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// Initial visibility when entering the rg pane.
    pub default_visibility: Visibility,
    /// Initial sort entering the rg pane.
    pub default_sort: Option<SortOrder>,
    /// Whether to display each match on a seperate line. This can be overridden with the --no-heading command line option.
    pub no_heading: bool,

    /// Template to display when searching with ripgrep
    pub rg_status_template: String,
    /// Template to display when filtering with fs
    pub fs_status_template: String,
}

impl Default for RgPaneSettings {
    fn default() -> Self {
        let mut default_visibility = Visibility::default();
        default_visibility.ignore = true;

        Self {
            prompt: None,
            show_preview: None,
            enter_prompt: true,
            preview_layout_index: 1,

            no_heading: false, // todo: lowpri: false or true?
            default_visibility,
            default_sort: Some(SortOrder::none),

            rg_status_template: r"{blue:filter: {}} \s\m/\t".into(),
            fs_status_template: r"{red:query: {}} \s\m/\t".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NavPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    pub default_sort: SortOrder,
    /// Default visibility when no visibility is specified.
    pub default_visibility: Visibility,
}

impl Default for NavPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            preview_layout_index: 0,

            default_sort: SortOrder::mtime,
            default_visibility: Default::default(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HistoryPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
    pub enter_prompt: bool,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
    pub enter_prompt: bool,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    pub app_scan_directories: Vec<PathBuf>,
}
