use cba::{bs::create_dir, vec_};
use std::{collections::HashMap, path::PathBuf};

use crate::{
    cli::clap_helpers::ClapStyleOverride, db::zoxide::HistoryConfig, watcher::WatcherConfig,
};
use crate::{
    cli::{CliOpts, paths::*},
    lessfilter::Preset,
};
use fist_types::When;

mod pager;
mod panes;
mod styles;
pub use pager::*;
pub use panes::*;
pub mod ui;
use ui::StyleConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SmartVisibilityConfig {
    /// When a find pattern starts with '.', automatically show hidden files.
    pub dot_query_show_hidden: bool,
    /// When a find pattern starts with '.', automatically show ignored files.
    pub dot_query_show_ignored: bool,
    /// Automatically enable `-h` (show hidden) in Nav panes if the directory contains only hidden entries, preventing the folder from appearing empty.
    pub empty_folders: bool,
    /// Automatically set hidden=true, ignore=true when inside a git repository.
    /// When::Auto (default) only applies smart git at init; When::Always also adapts visibility across git boundaries at runtime; When::Never disables it.
    pub git_repo: When,
    /// Disable -I (unhide ignored) if any explicitly provided path/target is ignored by git.
    pub ignored_targets: bool,
    /// Retain file/folder visibility filters (only files / only dirs) when changing to a nav pane.
    pub retain_filetype_filters: bool,
}

impl Default for SmartVisibilityConfig {
    fn default() -> Self {
        Self {
            dot_query_show_hidden: true,
            dot_query_show_ignored: false,
            empty_folders: true,
            git_repo: When::Auto,
            ignored_targets: true,
            retain_filetype_filters: false,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MatchmakerOverrides {
    pub fullscreen: bool,
    pub reverse: Option<bool>,
}

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

    /// Matchmaker overrides (`fullscreen`, `reverse`).
    #[serde(skip)]
    pub mm: MatchmakerOverrides,
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str(include_str!("../../assets/config/config.toml")).unwrap()
    }
}

impl std::ops::Deref for Config {
    type Target = GlobalConfig;
    fn deref(&self) -> &Self::Target {
        &self.global
    }
}

impl std::ops::DerefMut for Config {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.global
    }
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GlobalConfig {
    pub interface: InterfaceConfig,

    /// Configure behavior of filesystem actions.
    pub fs: FsConfig,

    /// Configure behavior of the fd tool.
    /// This affects [FsAction::Find](`crate::run::FsAction::Find`) and the default subcommand.
    pub fd: FdConfig,

    /// Configure behavior of the rg tool.
    /// This affects [FsAction::Rg](`crate::run::FsAction::Rg`) and the rg subcommand.
    pub rg: RgConfig,

    /// Configure various pane related settings.
    pub panes: PanesConfig,

    /// Configure background copy/move queue behavior.
    pub queue: QueueConfig,

    /// Configure smart visibility heuristics (dot-queries, empty folders, git-repo context, ignored targets).
    pub smart_visibility: SmartVisibilityConfig,
}

/// Settings for background transfer queue (copy/move).
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueueConfig {
    pub copy: fist_copy::TransferParams,
    pub r#move: fist_copy::TransferParams,
}

/// Miscellaneous and Tool specific options.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MiscConfig {
    /// How long to wait between consecutive clipboard actions
    pub clipboard_delay_ms: u64,
    /// Overwrite or append logs on application start.
    pub append_mode_logging: bool,
    /// Overwrite or append tool logs on application start.
    pub tools_append_mode_logging: bool,
    /// Pass the spawning command to this instead of invoking it directly.
    pub spawn_with: Vec<String>,
    pub list_absolute_paths: bool,
}

impl Default for MiscConfig {
    fn default() -> Self {
        Self {
            clipboard_delay_ms: 20,
            append_mode_logging: false,
            tools_append_mode_logging: false,
            spawn_with: Vec::new(),
            list_absolute_paths: false,
        }
    }
}

// -------------- GLOBAL --------------

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Settings related to the behavior of the main interface.
/// It is recommended not to change these.
pub struct InterfaceConfig {
    // actions
    /// The command template to execute when [FsAction::Advance](`crate::run::FsAction::Advance`) is invoked on a file.
    pub advance_command: String,
    /// If true, the functions of the Accept and Print actions will be swapped.
    pub alt_accept: bool,
    /// Disables multi-accept.
    pub no_multi_accept: bool,
    /// When outside the prompt, whether to register paste as characters or an action.
    pub always_paste: bool,
    /// Whether the Trash action operates on the underlying items in the
    /// database-backed panes (stashes and the files/folders/apps history).
    /// When false, Trash routes to Delete there: records are removed instead
    /// of the real paths.
    pub allow_trash_db_items: bool,
    /// Hide the preview while the cursor is disabled (locked onto the cwd).
    pub hide_preview_when_cursor_disabled: bool,
    /// When false, Parent inside an archive's extraction workdir leaves the
    /// archive (going to the directory that contains it) instead of
    /// exposing the internal unzip storage dir.
    pub allow_enter_unzip_directory: bool,
    /// Sorting stability for the match list of sorted panes: how tolerant
    /// the order is to score changes between reloads. Higher keeps the
    /// current order longer; 0 always re-sorts.
    pub stability_threshold: u32,
    /// When true, preserve the directory size cache on directory change if the
    /// target directory is already cached.
    pub preserve_size_cache: bool,

    // display
    /// The prefix to display when the cursor is in the prompt.
    pub cwd_prompt: String,
    /// Display a toast when current directory has no entries.
    pub toast_on_empty: bool,
    /// If [AutoJump](`crate::run::FsAction::AutoJump`) should accept or advance
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
            allow_trash_db_items: false,
            hide_preview_when_cursor_disabled: false,
            allow_enter_unzip_directory: false,
            stability_threshold: 30,
            preserve_size_cache: false,
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FdConfig {
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

    /// Query when no patterns are provided. Starting with '-v ' adds the -v flag.
    pub empty_pattern: Option<String>,
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
            empty_pattern: None,
            default_args: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FsConfig {
    /// Preserve selections across reloads: selections are saved as hashed
    /// absolute paths before the worker restart and rehydrated once the
    /// fresh listing lands (see [`crate::run::selection`]).
    pub refill_selections_after_reload: bool,
}

impl Default for FsConfig {
    fn default() -> Self {
        Self {
            refill_selections_after_reload: true,
        }
    }
}

// -------------- IMPL --------------------------

impl Config {
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
            ClapStyleOverride::IconColors => {
                style.file_icons = true;
                style.dir_icons = true;
                style.icon_colors = true;
                style.file_colors = true;
                style.dir_colors = true;
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

        if let Some(r) = cli.fullscreen {
            self.mm.fullscreen = true;
            self.mm.reverse = r.map(|s| !s);
        }
        if cli.alt_accept {
            self.global.interface.alt_accept = !self.global.interface.alt_accept
        }
    }

    // --------------------------------------------------

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
    pub fn tools_log_path(&self) -> PathBuf {
        self.state_dir.join(format!("{BINARY_FULL}.tools.log"))
    }

    pub fn check_dirs_or_exit(&self) {
        let dirs = [&self.state_dir, &self.cache_dir];

        for dir in dirs {
            log::debug!("checking: {dir:?}");
            if !create_dir(dir) {
                std::process::exit(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{lessfilter::LessfilterConfig, run::mm_config::MMConfig};

    use super::*;

    #[test]
    fn deserialize_configs() {
        let _: Config = toml::from_str(include_str!("../../assets/config/config.toml")).unwrap();
        let _: Config = toml::from_str(include_str!("../../assets/config/dev.toml")).unwrap();
        let _: LessfilterConfig =
            toml::from_str(include_str!("../../assets/config/lessfilter.toml")).unwrap();
        let _: LessfilterConfig =
            toml::from_str(include_str!("../../assets/config/lessfilter.dev.toml")).unwrap();
        let _: PagerConfig =
            toml::from_str(include_str!("../../assets/config/pager.toml")).unwrap();
        let _: PagerConfig =
            toml::from_str(include_str!("../../assets/config/pager.dev.toml")).unwrap();
        let _: MMConfig = toml::from_str(include_str!("../../assets/config/mm.toml")).unwrap();
        let _: MMConfig = toml::from_str(include_str!("../../assets/config/mm.dev.toml")).unwrap();
        let _: MMConfig = toml::from_str(include_str!("../../assets/config/mac.mm.toml")).unwrap();
    }
}
