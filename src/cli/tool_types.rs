use std::path::PathBuf;

use clap::{ArgGroup, Parser};

use crate::{
    db::{DbSortOrder, DbTable},
    lessfilter::Preset,
};

#[derive(Debug, Parser, Default, Clone)]
pub struct ShellCommand {
    /// Name for jump function.
    #[arg(long, default_value_t = String::from("z"))]
    pub z_name: String,
    /// Name for open function.
    #[arg(long, default_value_t = String::from("zz"))]
    pub zz_name: String,
    /// Command used by open function
    #[arg(long, default_value_t = format!("{} :tool lessfilter edit", std::env::current_exe().unwrap_or(PathBuf::from("fs")).file_name().unwrap().to_string_lossy()))]
    pub visual: String,
    /// Default sort order for the interactive jump menu
    #[arg(long, default_value_t = DbSortOrder::atime)]
    pub z_sort: DbSortOrder,
    /// Arguments passed to `fs ::` when z is invoked with a trailing `.`
    #[arg(long, default_value_t = String::from("-D"))]
    pub z_dot_args: String,
    /// Arguments passed to `fs ::` when z is invoked with a trailing `..` (experimental)
    #[arg(long, default_value_t = String::from(""))]
    pub z_slash_args: String,
    #[arg(long, default_value_t)]
    pub aliases: bool,
}

#[derive(Debug, Parser, Default, Clone)]
pub struct LessfilterCommand {
    #[arg(value_name = "PRESET")]
    pub preset: Preset,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub paths: Vec<PathBuf>,

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
