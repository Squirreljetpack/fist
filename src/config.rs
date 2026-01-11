use cli_boilerplate_automation::{
    bait::ResultExt,
    bath::{_filename, RenamePolicy},
    bo::write_str,
    bog::BogOkExt,
    bs::{create_dir, set_executable},
    ibog,
};
use std::{collections::HashMap, path::PathBuf};

use crate::{cli::BINARY_FULL, cli::paths::*, lessfilter::Preset};
use crate::{
    cli::paths::{liza_path, pager_path},
    db::zoxide::HistoryConfig,
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
    /// directory for storing history and other state.
    #[serde(default = "state_dir")]
    pub state_dir: PathBuf,

    /// cache directory.
    #[serde(default = "cache_dir")]
    pub cache_dir: PathBuf,

    /// A container for settings whose values are accessed at runtime.
    /// Its fields are included directly in (flattened into) the config.
    #[serde(flatten)]
    pub global: GlobalConfig,

    /// All styling options not governed by match-maker.
    pub styles: StyleConfig,

    /// Configure the filesystem watcher
    pub notify: WatcherConfig,

    /// Miscellaneous and Tool specific options
    pub misc: MiscConfig,

    /// Settings related to saving to and retrieving from history.
    pub history: HistoryConfig,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub interface: InterfaceConfig,

    /// Configure behavior of filesystem actions.
    #[serde(default)]
    pub fs: FsConfig,

    /// Configure behavior of the fd tool.
    /// This affects [FsAction::Find](`crate::run::fsaction::FsAction::Find`) and the default subcommand.
    #[serde(default)]
    pub fd: FdConfig,

    /// Configure behavior of filesystem actions.
    #[serde(default)]
    pub panes: PanesConfig,
}

impl Config {
    pub fn check_dirs_or_exit(&self) {
        let dirs = [&self.state_dir, &self.cache_dir];

        for dir in dirs {
            log::debug!("checking: {dir:?}");
            if !create_dir(dir) {
                std::process::exit(1)
            }
        }
    }

    // initialize helper files
    pub fn check_scripts(
        &self,
        force: bool,
    ) {
        let files = [
            (liza_path(), include_str!("../assets/scripts/liza")),
            (pager_path(), include_str!("../assets/scripts/pager")),
            (
                metadata_viewer_path(),
                include_str!("../assets/scripts/fist_metadata_viewer"),
            ),
            (
                binary_viewer_path(),
                include_str!("../assets/scripts/fist_binary_viewer"),
            ),
            (
                show_error_path(),
                include_str!("../assets/scripts/fist_show_error"),
            ),
        ];

        for (path, script) in files {
            let error_prefix = format!("Failed set executability of {path:?}");
            if (force || !path.exists())
                && write_str(path, script)._ebog().is_some()
                && set_executable(path).prefix(&error_prefix)._ebog().is_some()
            {
                if !force
                // less noise for debug
                {
                    ibog!("{} saved to: {}", _filename(path), path.to_string_lossy());
                }
            }
        }
    }
}

/// Miscellaneous and Tool specific options.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MiscConfig {
    /// How long to wait between consecutive clipboard actions
    pub clipboard_delay_ms: u64,
    /// When --cd is specified, whether to error or begin search when no match is found.
    pub cd_fallback_search: bool,
    /// Overwrite or append logs on application start.
    pub append_mode_logging: bool,
    /// Pass the spawning command to this instead of invoking it directly.
    // todo
    pub spawn_with: Vec<String>,
}

impl Default for MiscConfig {
    fn default() -> Self {
        Self {
            clipboard_delay_ms: 20,
            cd_fallback_search: false,
            append_mode_logging: false,
            spawn_with: Vec::new(),
        }
    }
}

// -------------- GLOBAL ---------------------------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Settings related to the behavior of the main interface.
/// It is recommended not to change these.
pub struct InterfaceConfig {
    // actions
    /// The command template to execute when [FsAction::Advance](`crate::run::fsaction::FsAction::Advance`) is invoked on a file.
    pub advance_command: String,
    /// If true, the functions of the Accept and Print actions will be swapped.
    pub alt_accept: bool,
    /// Disables multi-select.
    pub no_multi: bool,
    /// When outside the prompt, whether to register paste as characters or an action.
    pub always_paste: bool,

    // display
    /// The prefix to display when the cursor is in the prompt.
    pub cwd_prompt: String,
    /// Display a toast when current directory has no entries.
    pub toast_on_empty: bool, // todo
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            alt_accept: false,
            no_multi: false,
            always_paste: false,
            advance_command: Preset::Edit.to_command_string(),
            cwd_prompt: "{} ".into(),
            toast_on_empty: true,
        }
    }
}
// ------------- PANES ------------------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanesSettings {
    pub display_script_batch_size: usize,
}

impl Default for PanesSettings {
    fn default() -> Self {
        Self {
            display_script_batch_size: 15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Pane-specific settings
pub struct PanesConfig {
    pub app: PaneSettings,
    pub history: PaneSettings,
    pub nav: NavPaneSettings,
    pub stream: PaneSettings,
    pub fd: PaneSettings,
    pub custom: PaneSettings,
    pub rg: PaneSettings,

    pub settings: PanesSettings,
}

// enter prompt by default because it is less surprising
impl Default for PanesConfig {
    fn default() -> Self {
        Self {
            app: PaneSettings {
                show_preview: Some(false),
                ..PaneSettings::default()
            },
            history: PaneSettings {
                ..PaneSettings::default()
            },
            nav: NavPaneSettings::default(),
            fd: PaneSettings {
                ..PaneSettings::default()
            },
            rg: PaneSettings {
                ..PaneSettings::default()
            },
            custom: PaneSettings {
                ..PaneSettings::default()
            },
            stream: PaneSettings {
                ..PaneSettings::default()
            },

            settings: PanesSettings::default(),
        }
    }
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
            FsPane::Files { .. } | FsPane::Folders { .. } => panes.history.prompt.clone(),
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
            FsPane::Files { .. } | FsPane::Folders { .. } => panes.history.show_preview,
            FsPane::Launch { .. } => panes.app.show_preview,
            FsPane::Nav { .. } => panes.nav.show_preview,
            FsPane::Rg { .. } => panes.rg.show_preview,
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
}
impl Default for PaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            enter_prompt: true,
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
    pub enter_prompt: bool,
    // ----------------------------
    pub default_sort: SortOrder,
    pub default_visibility: Visibility,
}

impl Default for NavPaneSettings {
    fn default() -> Self {
        Self {
            prompt: None,
            show_preview: None,
            enter_prompt: false,

            default_sort: SortOrder::mtime,
            default_visibility: Default::default(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdConfig {
    /// A map of folders to exclusion globs which should be applied when in them.
    /// ~ can be used in lieu of $HOME.
    /// If a list is specified for the empty path "", that list will override the list of default exclusions for the platform, and apply everywhere.
    /// Only one value (exclusion list) can apply to each path.
    pub exclusions: HashMap<PathBuf, Vec<String>>,

    /// Arguments added to every fd command
    pub base_args: Vec<String>,
    /// When no path is given to fs, such as using `fs [pattern]`, whether to search in `$HOME` or the current directory.
    pub default_search_in_home: bool,
    //  ---------------- Experimental/Nonstandard ---------------
    /// When given a set of paths to search with `fs`
    pub reduce_paths: bool,
    /// The set of arguments applied to the end of `fs ::` when no `fd_args` were given.
    pub default_args: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FsConfig {
    pub rename_policy: RenamePolicy,
}

// #[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
// #[serde(default, deny_unknown_fields)]
// pub struct CurrentConfig {
//     pub render_script: Option<String>,

//     #[serde(deserialize_with = "escaped_opt_char")]
//     pub delimiter: Option<char>,
// }

impl Config {
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
