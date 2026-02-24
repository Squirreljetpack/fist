use cli_boilerplate_automation::{
    bait::ResultExt,
    bath::{PathExt, RenamePolicy},
    bo::write_str,
    bog::BogOkExt,
    bs::{create_dir, set_executable},
    ibog, vec_,
};
use std::{collections::HashMap, path::PathBuf};

use crate::{
    cli::{CliOpts, paths::*},
    lessfilter::Preset,
    spawn::menu_action::MenuActions,
};
use crate::{
    cli::{
        clap_helpers::ClapStyleOverride,
        paths::{liza_path, text_renderer_path},
    },
    db::zoxide::HistoryConfig,
    watcher::WatcherConfig,
};
use fist_types::When;

mod panes;
mod partial;
mod styles;
pub use panes::*;
pub use partial::*;
pub mod ui;
use ui::StyleConfig;
// ------ CONFIG ------
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// directory for storing history and other state.
    #[serde(default = "state_dir")]
    pub state_dir: PathBuf,

    /// cache directory.
    #[serde(default = "cache_dir")]
    pub cache_dir: PathBuf,

    /// A container for settings whose values are accessed at runtime.
    /// Its fields are included directly in (flattened into) the config.
    #[serde(flatten, default)]
    pub global: GlobalConfig,

    /// All styling options not governed by the match-maker cfg
    #[serde(default)]
    pub styles: StyleConfig,

    /// Configure the filesystem watcher
    #[serde(default)]
    pub notify: WatcherConfig,

    /// Miscellaneous and Tool specific options
    #[serde(default)]
    pub misc: MiscConfig,

    /// Settings related to saving to and retrieving from history.
    #[serde(default)]
    pub history: HistoryConfig,

    /// Custom actions which appear in the menu
    #[serde(default)]
    pub actions: MenuActions,
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str(include_str!("../../assets/config/config.toml")).unwrap()
    }
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalConfig {
    pub interface: InterfaceConfig,

    /// Configure behavior of filesystem actions.
    pub fs: FsConfig,

    /// Configure behavior of the fd tool.
    /// This affects [FsAction::Find](`crate::run::fsaction::FsAction::Find`) and the default subcommand.
    pub fd: FdConfig,

    /// Configure behavior of the rg tool.
    /// This affects [FsAction::Rg](`crate::run::fsaction::FsAction::Rg`) and the rg subcommand.
    pub rg: RgConfig,

    /// Configure various pane related settings.
    pub panes: PanesConfig,

    /// Matchmaker styling overrides for panes.
    pub mm: MatchmakerOverrides, // not sure about the role, eventually we want some pane-specific matchmaker overrides, should be stored
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
            (liza_path(), include_str!("../../assets/scripts/liza")),
            (
                text_renderer_path(),
                include_str!("../../assets/scripts/pager"),
            ),
            (
                show_error_path(),
                include_str!("../../assets/scripts/fist_show_error"),
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
                    ibog!("{} saved to: {}", path.filename(), path.to_string_lossy());
                }
            }
        }
    }

    pub fn override_from(
        &mut self,
        cli: &CliOpts,
    ) {
        let style = &mut self.styles.path;
        match cli.style {
            ClapStyleOverride::Auto => {
                // leave config unchanged
            }
            ClapStyleOverride::None => {
                style.file_icons = false;
                style.file_colors = false;
                style.dir_icons = false;
                style.dir_colors = false;
            }
            ClapStyleOverride::Icons => {
                style.file_icons = true;
                style.dir_icons = true;

                style.file_colors = false;
                style.dir_colors = false;
            }
            ClapStyleOverride::Colors => {
                style.file_icons = false;
                style.dir_icons = false;

                style.file_colors = true;
                style.dir_colors = true;
            }
            ClapStyleOverride::All => {
                style.file_icons = true;
                style.file_colors = true;
                style.dir_icons = true;
                style.dir_colors = true;
            }
        }

        self.global.mm.fullscreen |= cli.fullscreen
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
    /// Disables multi-accept.
    pub no_multi_accept: bool,
    /// When outside the prompt, whether to register paste as characters or an action.
    pub always_paste: bool,

    // display
    /// The prefix to display when the cursor is in the prompt.
    pub cwd_prompt: String,
    /// Display a toast when current directory has no entries.
    pub toast_on_empty: bool, // todo
    pub autojump_advance: bool,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            alt_accept: false,
            no_multi_accept: false,
            always_paste: false,
            advance_command: Preset::Edit.to_command_string(When::Auto),
            cwd_prompt: "{} ".into(),
            toast_on_empty: true,
            autojump_advance: false,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdConfig {
    // todo: lowpri: lookup is by OsString but storage is as String...
    /// A map of folders => exclusion globs which should be applied when in them.
    /// ~ can be used in lieu of $HOME.
    /// If a list is specified for the empty path "", that list will override the list of default exclusions for the platform, and apply everywhere.
    /// Only one value (exclusion list) can apply to each path.
    pub exclusions: HashMap<PathBuf, Vec<String>>,

    /// Arguments added to every fd command
    pub base_args: Vec<String>,

    /// When no path is given to fs, such as using `fs [pattern]`, whether to search in `$HOME` or the current directory.
    pub default_search_in_home: bool,
    /// Enabling this will hide ignored files when a pattern but no path is given to fs, such as using `fs [pattern]`, provided that ignore was not explicitly specified to the cli.
    pub default_search_ignore: bool,
    //  ---------------- Experimental/Nonstandard ---------------
    /// When given a set of paths to search with `fs`, change the working directory to their common denominator.
    pub reduce_paths: bool,
    /// The set of arguments applied to the end of `fs ::` when no `fd_args` were given.
    pub default_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RgConfig {
    /// A map of folders => globs which should be applied when in them.
    /// ~ can be used in lieu of $HOME.
    /// If a list is specified for the empty path "", that list will apply everywhere.
    /// Only one value can apply to each path.
    /// Multiple glob flags may be used. Globbing rules match .gitignore globs. Precede a glob with a ! to exclude it. If multiple globs match a file or directory, the glob given later in the command line takes precedence. Globs used via this flag are matched case insensitively. This is passed on to rg through the `--iglob` parameter.
    pub iglobs: HashMap<PathBuf, Vec<String>>,
    /// Arguments added to every rg command
    pub base_args: Vec<String>,

    //  ---------------- Experimental/Nonstandard ---------------
    /// The set of arguments applied to the end of `fs :` when no `rg_args` were given.
    pub default_args: Vec<String>,
}

impl Default for RgConfig {
    fn default() -> Self {
        RgConfig {
            iglobs: Default::default(),
            base_args: vec_![
                "--trim",
                "--color=ansi",
                "--no-context-separator",
                "--field-context-separator=-",
            ],
            default_args: Default::default(),
        }
    }
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
