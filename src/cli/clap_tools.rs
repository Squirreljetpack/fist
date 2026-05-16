use std::{ffi::OsString, path::PathBuf};

use clap::{ArgAction, ArgGroup, Parser};

use crate::{db::DbTable, lessfilter::Preset};

#[derive(Debug, Parser, Default, Clone)]
pub struct ShellCommand {
    /// Name for jump function.
    #[arg(long, default_value_t = String::from("z"))]
    pub z_name: String,
    /// Arguments passed to `fs ::` when z is invoked with a trailing `.`
    #[arg(long, default_value_t = String::from("-F --style=colors"))]
    pub z_dot_args: String,
    /// Arguments passed to `fs ::` when z is invoked with a trailing `./`
    #[arg(long, default_value_t = String::from(""))]
    pub z_slash_args: String,
    /// Arguments passed to `fs :dir` for the interactive jump menu
    #[arg(long, default_value_t = String::from("--sort atime --style=colors --enter-prompt=false"))]
    pub z_dir_args: String,

    /// Name for open function.
    #[arg(long, default_value_t = String::from("zz"))]
    pub open_name: String,
    /// Command used by open function
    #[arg(long, default_value_t = format!("{} :tool lessfilter edit", std::env::current_exe().unwrap_or(PathBuf::from("fs")).file_name().unwrap().to_string_lossy()))]
    pub open_cmd: String,

    /// Bind for the directory widget.
    #[arg(long, default_value_t = String::from("^[[1;2D"))]
    pub dir_widget_bind: String,
    /// Bind for the directory widget.
    #[arg(long, default_value_t = String::from("^[[1;2C"))]
    pub file_widget_bind: String,
    /// Bind for the directory widget.
    #[arg(long, default_value_t = String::from("^[[1;2B"))]
    pub rg_widget_bind: String,

    #[arg(long)]
    pub file_open_cmd: Option<String>,
    #[arg(long)]
    pub rg_open_cmd: Option<String>,
    /// Arguments passed to `fs ::` when dir widget is invoked
    #[arg(long, default_value_t = String::from("-F --style=colors --enter-prompt=false"))]
    pub dir_widget_args: String,
    /// Arguments passed to `fs ::` when file widget is invoked
    #[arg(long, default_value_t = String::from("--alt-accept -f --style=icon-colors --enter-prompt=false  -- .."))]
    pub file_widget_args: String,
    /// Arguments passed to `fs :` when rg widget is invoked
    #[arg(long, default_value_t = String::from("-1 --fullscreen --style=colors --preserve-whitespace"))]
    pub rg_widget_args: String,

    #[arg(long, default_value_t)]
    pub aliases: bool,
    /// Name for the nav function.
    #[arg(long, default_value_t = String::from("Z"))]
    pub nav_name: String,

    #[arg(long)]
    pub shell: Option<String>,
}

#[derive(Debug, Parser, Default, Clone)]
pub struct LessfilterCommand {
    #[arg(value_name = "PRESET")]
    pub preset: Preset,

    /// Arguments to pass to the first executed command (experimental).
    #[arg(short = 'a', long = "arg")]
    pub args: Vec<OsString>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub paths: Vec<PathBuf>,

    #[arg(long, action = ArgAction::SetTrue)]
    pub no_exec: bool,

    #[arg(long, action = ArgAction::SetTrue)]
    pub tty: bool,

    // Some(true) => Always show header at top
    // Some(false) => Never show header
    // None => action-dependent
    #[arg(long, action = clap::ArgAction::Set)]
    pub header: Option<bool>,
}

#[derive(Debug, Parser, Clone)]
#[command(group(
    ArgGroup::new("target")
    .required(true)
    .args(["paths", "glob", "reset"])
))]
pub struct BumpCommand {
    /// path to bump.
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// glob pattern to bump.
    #[arg(short, long)]
    pub glob: Option<String>,

    /// amount to bump by, 0 to clear.
    #[arg(short, long, default_value_t = 1)]
    pub count: i32,

    /// reset the database.
    #[arg(long)]
    pub reset: bool,

    /// table matched on by the glob.
    #[arg(last(true))]
    pub table: Option<DbTable>,
}

#[derive(Debug, Parser, Default, Clone)]
pub struct TypesCommand {}
