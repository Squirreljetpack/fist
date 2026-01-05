use cli_boilerplate_automation::{
    bait::ResultExt,
    bath::{RenamePolicy, basename},
    bo::write_str,
    bog::BogOkExt,
    bs::{create_dir, set_executable},
    ibog,
};
use std::{collections::HashMap, path::PathBuf};

use super::{
    BINARY_FULL,
    paths::{cache_dir, state_dir},
};
use crate::{
    cli::paths::{binary_viewer_path, header_viewer_path, metadata_viewer_path},
    lessfilter::Preset,
    utils::serde::escaped_opt_char,
};
use crate::{
    cli::paths::{lz_path, pager_path},
    db::zoxide::DbConfig,
    filters::*,
    run::FsPane,
    ui::styles_config::StyleConfig,
    watcher::WatcherConfig,
};
// ------ CONFIG ------
// default is placed on individual fields to protect against panics on invalid defaults
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(flatten)]
    pub global: GlobalConfig,

    pub styles: StyleConfig,

    pub notify: WatcherConfig,

    pub misc: MiscConfig,

    pub db: DbConfig,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GlobalConfig {
    // not sure these need be in global
    /// store history
    #[serde(default = "state_dir")]
    pub state_dir: PathBuf,

    /// cache
    #[serde(default = "cache_dir")]
    pub cache_dir: PathBuf,

    #[serde(default)]
    pub interface: InterfaceConfig,

    #[serde(default)]
    pub fs: FsConfig,

    #[serde(default)]
    pub fd: FdConfig,

    #[serde(default)]
    pub current: CurrentConfig,

    #[serde(default)]
    pub panes: PanesConfig,
}

impl Config {
    pub fn check_dirs_or_exit(&self) {
        let dirs = [&self.global.state_dir, &self.global.cache_dir];

        for dir in dirs {
            log::debug!("checking: {dir:?}");
            if !create_dir(dir) {
                std::process::exit(1)
            }
        }
    }

    pub fn check_files(&self) {
        let files = [
            (lz_path(), include_str!("../../assets/scripts/lz")),
            (pager_path(), include_str!("../../assets/scripts/pager")),
            (
                metadata_viewer_path(),
                include_str!("../../assets/scripts/fist_metadata_viewer"),
            ),
            (
                binary_viewer_path(),
                include_str!("../../assets/scripts/fist_binary_viewer"),
            ),
            (
                header_viewer_path(),
                include_str!("../../assets/scripts/fist_header_viewer"),
            ),
        ];

        for (path, script) in files {
            let error_prefix = format!("Failed set executability of {path:?}");
            if !path.exists()
                && write_str(path, script)._ebog().is_some()
                && set_executable(path).prefix(&error_prefix)._ebog().is_some()
            {
                ibog!("{} saved to: {}", basename(path), path.to_string_lossy());
            }
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.global.db_path()
    }

    pub fn log_path(&self) -> PathBuf {
        self.global.log_path()
    }
}

/// Miscellaneous + Tool specific options
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MiscConfig {
    pub clipboard_delay_ms: u64,
    pub cd_fallback_search: bool,
}

impl Default for MiscConfig {
    fn default() -> Self {
        Self {
            clipboard_delay_ms: 20,
            cd_fallback_search: false,
        }
    }
}

// -------------- GLOBAL ---------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InterfaceConfig {
    pub alt_accept: bool,
    pub enter_cmd: String,
    pub cwd_prompt: String,
    // When outside the prompt, whether to register paste as characters or an action.
    pub always_paste: bool,
    pub toast_on_empty: bool,

    // experimental
    pub default_sort: Option<SortOrder>,
    pub default_visibility: Option<Visibility>,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            alt_accept: false,
            always_paste: false,
            enter_cmd: Preset::Edit.to_command_string(),
            default_sort: None,
            default_visibility: None,
            cwd_prompt: "{} ".into(),
            toast_on_empty: true,
        }
    }
}
// ------------- PANES ------------------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanesConfig {
    pub app: PaneSettings,
    pub file: PaneSettings,
    pub dir: PaneSettings,
    pub nav: NavPaneSettings,
    pub stream: PaneSettings,
    pub fd: PaneSettings,
    pub custom: PaneSettings,
    pub rg: PaneSettings,
}

impl FsPane {
    pub fn prompt(
        &self,
        panes: &PanesConfig,
    ) -> Option<String> {
        match self {
            FsPane::Custom { .. } => panes.custom.prompt.clone(),
            FsPane::Stream { .. } => panes.stream.prompt.clone(),
            FsPane::Fd { .. } => panes.fd.prompt.clone(),
            FsPane::Files { .. } => panes.file.prompt.clone(),
            FsPane::Folders { .. } => panes.dir.prompt.clone(),
            FsPane::Launch { .. } => panes.app.prompt.clone(),
            FsPane::Nav { .. } => panes.nav.prompt.clone(),
            FsPane::Rg { .. } => panes.rg.prompt.clone(),
        }
    }

    pub fn preview_show(
        &self,
        panes: &PanesConfig,
    ) -> Option<bool> {
        match self {
            FsPane::Custom { .. } => panes.custom.show_preview,
            FsPane::Stream { .. } => panes.stream.show_preview,
            FsPane::Fd { .. } => panes.fd.show_preview,
            FsPane::Files { .. } => panes.file.show_preview,
            FsPane::Folders { .. } => panes.dir.show_preview,
            FsPane::Launch { .. } => panes.app.show_preview,
            FsPane::Nav { .. } => panes.nav.show_preview,
            FsPane::Rg { .. } => panes.rg.show_preview,
        }
    }
}
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NavPaneSettings {
    /// Input prompt
    pub prompt: Option<String>,
    /// Whether to show the preview when switching to this pane. (Default: inherit).
    pub show_preview: Option<bool>,

    pub default_sort: SortOrder,
    pub default_visibility: Visibility,
}

impl Default for NavPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            default_sort: SortOrder::mtime,
            default_visibility: Default::default(),
        }
    }
}

impl Default for PanesConfig {
    fn default() -> Self {
        Self {
            app: PaneSettings {
                show_preview: Some(false),
                ..PaneSettings::default()
            },
            file: PaneSettings::default(),
            dir: PaneSettings::default(),
            nav: NavPaneSettings::default(),
            fd: PaneSettings::default(),
            rg: PaneSettings::default(),
            custom: PaneSettings::default(),
            stream: PaneSettings::default(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdConfig {
    pub exclusions: HashMap<PathBuf, Vec<String>>,
    pub default_args: Vec<String>,
    pub base_args: Vec<String>,
    // pub default_args_file: Option<PathBuf>,
    pub reduce_paths: bool,
    pub default_search_in_home: bool,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FsConfig {
    pub rename_policy: RenamePolicy,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CurrentConfig {
    pub no_multi: bool,

    #[serde(default, deserialize_with = "escaped_opt_char")]
    pub delimiter: Option<char>,
}

impl GlobalConfig {
    pub fn db_path(&self) -> PathBuf {
        #[cfg(debug_assertions)]
        {
            self.state_dir.join("dev.db")
        }

        #[cfg(not(debug_assertions))]
        {
            self.state_dir.join("record.db")
        }
    }
    pub fn log_path(&self) -> PathBuf {
        self.state_dir.join(format!("{BINARY_FULL}.log"))
    }
}

// ----------------------------------------
