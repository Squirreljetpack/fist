use std::{collections::HashMap, path::PathBuf};

use crate::run::FsPane;
use fist_types::filters::*;
use matchmaker::config::ShowCondition;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanesSettings {
    pub display_script_simultaneous_count: usize,
    pub display_script_batch_size: usize,
    /// Change the sorting method to the new default when changing to a new pane type
    pub apply_default_sort: bool,
}

impl Default for PanesSettings {
    fn default() -> Self {
        Self {
            display_script_simultaneous_count: 15,
            display_script_batch_size: 1000,
            apply_default_sort: true,
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
    pub find: FdPaneSettings,
    pub search: RgPaneSettings,
    pub custom: PaneSettings,
    pub stashes: StashPaneSettings,

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
            stashes: StashPaneSettings::default(),

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
    pub lock_prompt: Option<bool>,

    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
}
// impl Default for PaneSettings {
//     fn default() -> Self {
//         Self {
//             prompt: None,
//             show_preview: None,
//             lock_prompt: Some(true),
//             preview_layout_index: 0,
//         }
//     }
// }

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Whether to enter the prompt when switching to this pane
    pub lock_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// Default visibility.
    /// - When None: show hidden files and hide ignored files when inside a git repository and the inverse otherwise
    pub default_visibility: Option<PartialVisibility>,
    /// When leaving the fd pane, untoggle the `only show directories` visibility filter.
    pub on_leave_unset_dirs_only: bool,
    /// If the number of items added is less than this threshold, enable the directory watcher to auto-refresh the pane on changes.
    pub max_refresh_items_threshold: usize,
    /// If the execution time is less than this threshold (in milliseconds), enable the directory watcher to auto-refresh the pane on changes.
    #[serde(with = "crate::watcher::serde_duration_ms")]
    pub max_refresh_execution_time_threshold: std::time::Duration,
}

impl Default for FdPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            lock_prompt: None,
            preview_layout_index: 0,
            default_visibility: None,
            on_leave_unset_dirs_only: false,
            max_refresh_items_threshold: 20000,
            max_refresh_execution_time_threshold: std::time::Duration::from_millis(400), // a generous default threshold to be sure it's working
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RgPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Whether to enter the prompt when switching to this pane
    pub lock_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// Initial visibility when entering the rg pane.
    ///
    /// - When None: show hidden files and hide ignored files when inside a git repository and the inverse otherwise
    pub default_visibility: Option<PartialVisibility>,
    /// Initial sort entering the rg pane.
    pub default_sort: Option<SortOrder>,
    #[serde(alias = "no_heading")]
    /// Whether to display each match on a seperate line. This can be overridden with the --one-line command line option.
    pub one_line: bool,
    /// Whether to search fixed strings by default. This can be overridden on the command line.
    pub fixed_strings: bool,

    /// Append a "'" to the query start to prevent splitting it into multiple patterns by whitespace
    pub preserve_whitespace: bool,

    /// Template to display when searching with ripgrep
    pub rg_status_template: String,
    /// Template to display when filtering with fs
    pub fs_status_template: String,
}

// impl Default for RgPaneSettings {
//     fn default() -> Self {
//         Self {
//             prompt: None,
//             lock_prompt: Some(true),
//             show_preview: Some(ShowCondition::Free(20)),
//             preview_layout_index: 1,

//             one_line: true,
//             fixed_strings: false,
//             default_visibility: None,
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
    pub lock_prompt: Option<bool>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,

    // ----------------------------
    pub default_sort: Option<SortOrder>,
    /// Default visibility.
    /// - When None: show hidden files and hide ignored files when inside a git repository and the inverse otherwise
    pub default_visibility: Option<PartialVisibility>,
}

impl Default for NavPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            lock_prompt: None,
            show_preview: Some(ShowCondition::Free(50)),
            preview_layout_index: 0,

            default_sort: Some(SortOrder::mtime),
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
    pub lock_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
}

/// What to do when stashing a path that is already present in the stash.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertionStrategy {
    /// Remove the existing entry and add a fresh one, moving the path to
    /// the end of the stash (newest add time).
    #[default]
    Replace,
    /// Keep the existing entry; do not add another one.
    Skip,
    /// Add another entry even if the path is already stashed.
    Duplicate,
}

/// Per-stash pane settings, keyed by stash name (the unnamed stash is "").
pub type StashPaneSettings = HashMap<String, StashPaneSetting>;

/// How a stash pane treats its entries.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StashPaneKind {
    /// While populating, delete entries whose path no longer exists from
    /// the db (in addition to hiding them).
    Prune,
    /// While populating, hide entries whose path no longer exists.
    Filter,
    /// An in-memory stash: starts empty each run and entries are shown as
    /// stored while populating.
    #[default]
    Transient,
}

/// Settings of a single stash pane, looked up by stash name.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StashPaneSetting {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    /// Whether to enter the prompt when switching to this pane
    pub lock_prompt: Option<bool>,
    /// Default preview layout index for this pane
    pub preview_layout_index: u8,
    // ----------------------------
    /// How the stash treats its entries while populating.
    pub kind: StashPaneKind,
    /// What to do when stashing a path that is already in the stash.
    pub insert: InsertionStrategy,
}

impl StashPaneSetting {
    /// Fallback applied to stash panes without a configured entry: kind
    /// Transient, insert Replace.
    pub const DEFAULT: Self = Self {
        prompt: None,
        show_preview: None,
        lock_prompt: None,
        preview_layout_index: 0,
        kind: StashPaneKind::Transient,
        insert: InsertionStrategy::Replace,
    };
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<ShowCondition>,
    pub lock_prompt: Option<bool>,
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
            FsPane::Find { .. } => self.find.prompt.clone(),
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.prompt.clone(),
            FsPane::Apps { .. } => self.app.prompt.clone(),
            FsPane::Nav { .. } => self.nav.prompt.clone(),
            FsPane::Search { .. } => self.search.prompt.clone(),
            FsPane::Stash { stash_name, .. } => self.stash_setting(stash_name).prompt.clone(),
        }
    }

    pub fn locks_prompt(
        &self,
        pane: &FsPane,
    ) -> Option<bool> {
        match pane {
            FsPane::Custom { .. } => self.custom.lock_prompt,
            FsPane::Find { .. } => self.find.lock_prompt,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.lock_prompt,
            FsPane::Apps { .. } => self.app.lock_prompt,
            FsPane::Nav { .. } => self.nav.lock_prompt,
            FsPane::Search { .. } => self.search.lock_prompt,
            FsPane::Stash { stash_name, .. } => self.stash_setting(stash_name).lock_prompt,
        }
    }

    pub fn show_preview(
        &self,
        pane: &FsPane,
    ) -> Option<ShowCondition> {
        match pane {
            FsPane::Custom { .. } => self.custom.show_preview,
            FsPane::Find { .. } => self.find.show_preview,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.show_preview,
            FsPane::Apps { .. } => self.app.show_preview,
            FsPane::Nav { .. } => self.nav.show_preview,
            FsPane::Search { .. } => self.search.show_preview,
            FsPane::Stash { stash_name, .. } => self.stash_setting(stash_name).show_preview,
        }
    }

    pub fn default_visibility(
        &self,
        pane: &FsPane,
    ) -> Option<PartialVisibility> {
        match pane {
            // todo: lowpri: maybe we aggregate more than just apps later, and add visibility
            FsPane::Custom { .. }
            | FsPane::Apps { .. }
            | FsPane::Files { .. }
            | FsPane::Folders { .. }
            | FsPane::Stash { .. } => None,
            FsPane::Find { .. } => self.find.default_visibility,
            FsPane::Nav { .. } => self.nav.default_visibility,
            FsPane::Search { .. } => self.search.default_visibility,
        }
    }

    pub fn default_sort(
        &self,
        pane: &FsPane,
    ) -> Option<SortOrder> {
        match pane {
            // todo: lowpri: maybe we aggregate more than just apps later, and add visibility
            FsPane::Nav { .. } => self.nav.default_sort,
            FsPane::Search { .. } => self.search.default_sort,
            _ => None,
        }
    }

    pub fn preview_layout_index(
        &self,
        pane: &FsPane,
    ) -> u8 {
        match pane {
            FsPane::Custom { .. } => self.custom.preview_layout_index,
            FsPane::Find { .. } => self.find.preview_layout_index,
            FsPane::Files { .. } | FsPane::Folders { .. } => self.history.preview_layout_index,
            FsPane::Apps { .. } => self.app.preview_layout_index,
            FsPane::Nav { .. } => self.nav.preview_layout_index,
            FsPane::Search { .. } => self.search.preview_layout_index,
            FsPane::Stash { stash_name, .. } => self.stash_setting(stash_name).preview_layout_index,
        }
    }

    /// Settings for the stash pane `name`. Stashes without an entry fall
    /// back to the default setting.
    pub fn stash_setting(
        &self,
        name: &str,
    ) -> &StashPaneSetting {
        self.stashes.get(name).unwrap_or(&StashPaneSetting::DEFAULT)
    }
}
