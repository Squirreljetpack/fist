use std::path::PathBuf;

use crate::run::FsPane;
use fist_types::filters::*;
use matchmaker::config::ShowCondition;

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
    pub find: FdPaneSettings,
    pub search: RgPaneSettings,
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
            find: FdPaneSettings {
                ..Default::default()
            },
            search: RgPaneSettings {
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

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: Option<bool>,

    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
}
// impl Default for PaneSettings {
//     fn default() -> Self {
//         Self {
//             prompt: None,
//             show_preview: None,
//             enter_prompt: Some(true),
//             preview_layout_index: 0,
//         }
//     }
// }

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// Default visibility when no visibility is specified.
    pub default_visibility: PartialVisibility,
    /// When leaving the fd pane, untoggle the `only show directories` visibility filter.
    pub on_leave_unset_dirs_only: bool,
}

// impl Default for FdPaneSettings {
//     fn default() -> Self {
//         Self {
//             prompt: None,
//             show_preview: Some(ShowCondition::Free(60)),
//             enter_prompt: None,
//             preview_layout_index: 0,

//             default_visibility: Default::default(),
//             on_leave_unset_dirs_only: false,
//         }
//     }
// }

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RgPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// Initial visibility when entering the rg pane.
    pub default_visibility: PartialVisibility,
    /// Initial sort entering the rg pane.
    pub default_sort: Option<SortOrder>,
    /// Whether to display each match on a seperate line. This can be overridden with the --no-heading command line option.
    pub no_heading: bool,
    /// Whether to search fixed strings by default. This can be overridden on the command line.
    pub fixed_strings: bool,

    /// Template to display when searching with ripgrep
    pub rg_status_template: String,
    /// Template to display when filtering with fs
    pub fs_status_template: String,

    /// Whether to display results on empty query
    pub search_empty_query: bool,
}

// impl Default for RgPaneSettings {
//     fn default() -> Self {
//         let default_visibility = PartialVisibility {
//             ignore: Some(true),
//             ..Default::default()
//         };

//         Self {
//             prompt: None,
//             enter_prompt: Some(true),
//             show_preview: Some(ShowCondition::Free(20)),
//             preview_layout_index: 1,

//             no_heading: true,
//             fixed_strings: false,
//             default_visibility,
//             default_sort: Some(SortOrder::none),
//             search_empty_query: true,

//             rg_status_template: r"{blue:filter: {}} \s\m/\t".into(),
//             fs_status_template: r"{red:query: {}} \s\m/\t".into(),
//         }
//     }
// }

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NavPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to enter the prompt when switching to this pane
    pub enter_prompt: Option<bool>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,

    // ----------------------------
    pub default_sort: SortOrder,
    /// Default visibility when no visibility is specified.
    pub default_visibility: PartialVisibility,
}

impl Default for NavPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            enter_prompt: None,
            show_preview: Some(ShowCondition::Free(50)),
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
    pub show_preview: Option<ShowCondition>,
    pub enter_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    pub enter_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    pub app_scan_directories: Vec<PathBuf>,
}

// -------------------------------------------------------------------

impl PanesConfig {
    pub fn prompt(
        &self,
        pane: &FsPane,
    ) -> Option<String> {
        match pane {
            FsPane::Custom { .. } => self.custom.prompt.clone(),
            FsPane::Stream { .. } => self.stream.prompt.clone(),
            FsPane::Find { .. } => self.find.prompt.clone(),
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.prompt.clone(),
            FsPane::Apps { .. } => self.app.prompt.clone(),
            FsPane::Nav { .. } => self.nav.prompt.clone(),
            FsPane::Search { .. } => self.search.prompt.clone(),
        }
    }

    pub fn enter_prompt(
        &self,
        pane: &FsPane,
    ) -> Option<bool> {
        match pane {
            FsPane::Custom { .. } => self.custom.enter_prompt,
            FsPane::Stream { .. } => self.stream.enter_prompt,
            FsPane::Find { .. } => self.find.enter_prompt,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.enter_prompt,
            FsPane::Apps { .. } => self.app.enter_prompt,
            FsPane::Nav { .. } => self.nav.enter_prompt,
            FsPane::Search { .. } => self.search.enter_prompt,
        }
    }

    pub fn show_preview(
        &self,
        pane: &FsPane,
    ) -> Option<ShowCondition> {
        match pane {
            FsPane::Custom { .. } => self.custom.show_preview,
            FsPane::Stream { .. } => self.stream.show_preview,
            FsPane::Find { .. } => self.find.show_preview,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.show_preview,
            FsPane::Apps { .. } => self.app.show_preview,
            FsPane::Nav { .. } => self.nav.show_preview,
            FsPane::Search { .. } => self.search.show_preview,
        }
    }

    pub fn default_visibility(
        &self,
        pane: &FsPane,
    ) -> Option<PartialVisibility> {
        match pane {
            // todo: lowpri: maybe we aggregate more than just apps later, and add visibility
            FsPane::Custom { .. }
            | FsPane::Stream { .. }
            | FsPane::Apps { .. }
            | FsPane::Files { .. }
            | FsPane::Folders { .. } => None,
            FsPane::Find { .. } => Some(self.find.default_visibility),
            FsPane::Nav { .. } => Some(self.nav.default_visibility),
            FsPane::Search { .. } => Some(self.search.default_visibility),
        }
    }

    pub fn preview_layout_index(
        &self,
        pane: &FsPane,
    ) -> u8 {
        match pane {
            FsPane::Custom { .. } => self.custom.preview_layout_index,
            FsPane::Stream { .. } => self.stream.preview_layout_index,
            FsPane::Find { .. } => self.find.preview_layout_index,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.preview_layout_index,
            FsPane::Apps { .. } => self.app.preview_layout_index,
            FsPane::Nav { .. } => self.nav.preview_layout_index,
            FsPane::Search { .. } => self.search.preview_layout_index,
        }
    }
}
