use cba::bs::create_dir;
use std::path::PathBuf;

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
    /// Keep completed rows in the queue overlay after it closes. When
    /// false (default), completed shared rows are cleared on close.
    pub clear_completed_on_close: bool,
}

/// Miscellaneous and Tool specific options.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MiscConfig {
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
            append_mode_logging: false,
            tools_append_mode_logging: false,
            spawn_with: Vec::new(),
            list_absolute_paths: false,
        }
    }
}

// -------------- GLOBAL --------------

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RemoveDbRecordAction {
    #[default]
    None,
    Trash,
    Delete,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
/// Settings related to the behavior of the main interface.
/// It is recommended not to change these without cause.
pub struct InterfaceConfig {
    // actions
    /// The command template to execute when [FsAction::Advance](`crate::run::FsAction::Advance`) is invoked on a file.
    pub advance_command: String,
    /// If true, the functions of the Accept and Print actions will be swapped.
    pub alt_accept: bool,
    /// If true, a print emits the current item instead of the selection; a sink
    /// that consumes paths one at a time sets this.
    pub no_multi_print: bool,
    /// When outside the prompt, whether to register paste as characters or an action.
    pub always_paste: bool,
    /// Which action deletes records in database-backed panes (stashes and the files/folders/apps history).
    /// When `Trash`, Trash removes the database record while Delete operates on the real path.
    /// When `Delete`, Delete removes the database record while Trash operates on the real path.
    /// When `None` (default), neither removes database records: both operate on the real paths.
    pub remove_db_record_action: RemoveDbRecordAction,
    /// Hide the preview while the cursor is disabled (locked onto the cwd).
    pub hide_preview_when_cursor_disabled: bool,
    /// When false, Parent inside a preview extraction leaves the archive
    /// (going to the directory that contains it) instead of exposing the
    /// per-process extraction root.
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
    /// If [JumpOrAccept](`crate::run::FsAction::JumpOrAccept`) should accept or
    /// advance; ignored by `JumpOrAccept(0)`, which always accepts
    pub jump_or_advance: bool,

    // filesystem actions
    /// Preserve selections across reloads: selections are saved as hashed
    /// absolute paths before the worker restart and rehydrated once the
    /// fresh listing lands (see [`crate::run::selection`]).
    pub refill_selections_after_reload: bool,
    /// Copy the selected files themselves to the system clipboard, so they can
    /// be pasted into a file manager. When false, Copy only enqueues to the
    /// queue.
    pub copy_files_to_clipboard: bool,
    /// Cut the selected files to the system clipboard on Move, so they can be
    /// pasted into a file manager. When false, Move only enqueues to the queue.
    pub move_files_to_clipboard: bool,
    /// Bump the paths a menu action is triggered on, including actions that
    /// only enqueue their targets.
    pub bump_menu_action_paths: bool,
    /// Bump the destinations of successful Copy, Move, and Symlink runs.
    pub bump_transfer_destinations: bool,
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            alt_accept: false,
            no_multi_print: false,
            always_paste: false,
            advance_command: Preset::Edit.to_command_string(When::Auto, false),
            cwd_prompt: "{=} ".into(),
            toast_on_empty: true,
            jump_or_advance: false,
            remove_db_record_action: RemoveDbRecordAction::None,
            hide_preview_when_cursor_disabled: false,
            allow_enter_unzip_directory: false,
            stability_threshold: 30,
            preserve_size_cache: false,
            refill_selections_after_reload: true,
            copy_files_to_clipboard: true,
            move_files_to_clipboard: false,
            bump_menu_action_paths: false,
            bump_transfer_destinations: false,
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
    /// Log file for the stderr of spawned commands.
    pub fn command_log_path(&self) -> PathBuf {
        self.state_dir.join(format!("{BINARY_FULL}.commands.log"))
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
    fn test_flatten_deny_unknown_fields() {
        use crate::config::panes::{FindPaneSettings, SearchPaneSettings};

        // Known find pane fields + flattened fd fields deserialize correctly
        let find_ok: FindPaneSettings = toml::from_str(
            "show_preview = 60\nexclusions = [\"node_modules\"]\nbase_args = [\"-H\"]",
        )
        .unwrap();
        assert_eq!(find_ok.fd.exclusions, vec!["node_modules"]);
        assert_eq!(find_ok.fd.base_args, vec!["-H"]);

        // Unknown field on FindPaneSettings must be rejected
        let find_err = toml::from_str::<FindPaneSettings>("show_preview = 60\nunknown_field = 123");
        assert!(
            find_err.is_err(),
            "Expected error on unknown field in FindPaneSettings"
        );

        // Known search pane fields + flattened rg fields deserialize correctly
        let search_ok: SearchPaneSettings =
            toml::from_str("one_line = true\nempty_pattern = \"-v ^ *$\"").unwrap();
        assert!(search_ok.one_line);
        assert_eq!(search_ok.rg.empty_pattern.as_deref(), Some("-v ^ *$"));

        // Unknown field on SearchPaneSettings must be rejected
        let search_err =
            toml::from_str::<SearchPaneSettings>("one_line = true\nunknown_field = 123");
        assert!(
            search_err.is_err(),
            "Expected error on unknown field in SearchPaneSettings"
        );
    }

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
