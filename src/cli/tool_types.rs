use std::{ffi::OsString, path::PathBuf};

use clap::{ArgGroup, Parser, Subcommand};
use cli_boilerplate_automation::bath::PathExt;

use crate::{
    db::{DbSortOrder, DbTable},
    lessfilter::Preset,
};

#[non_exhaustive]
#[derive(Subcommand, Debug, Clone, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum SubTool {
    Colors,
    /// List directory (eza wrapper)
    Liza {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Dump the initialization code for your shell
    Shell {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Context and preset dependent file handler
    Lessfilter {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Bump history entries
    Bump {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// List mappings supported by the --type parameter.
    Types {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
}

#[derive(Debug, Parser, Default, Clone)]
pub struct ShellCommand {
    /// Name for jump function.
    #[arg(long, default_value_t = String::from("z"))]
    pub z_name: String,
    /// Name for open function.
    #[arg(long, default_value_t = String::from("zz"))]
    pub zz_name: String,
    /// Command used by open function
    #[arg(long, default_value_t = format!("{} :tool lessfilter edit", crate::cli::paths::current_exe().basename()))]
    pub visual: String,
    /// Default sort order for the interactive jump menu
    #[arg(long, default_value_t = DbSortOrder::atime)]
    pub z_sort: DbSortOrder,
    /// Default sort order the `z .` menu
    #[arg(long, default_value_t = String::from("-t d"))]
    pub z_dot_args: String,
    #[arg(long, default_value_t)]
    pub aliases: bool,
}

#[derive(Debug, Parser, Default, Clone)]
pub struct LessfilterCommand {
    #[arg(value_name = "PRESET")]
    pub preset: Preset,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub paths: Vec<PathBuf>,

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
