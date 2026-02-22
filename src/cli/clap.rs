use std::{ffi::OsString, path::PathBuf};

use clap::{ArgAction, Parser, Subcommand, error::ErrorKind};

use crate::{
    cli::{
        SubTool,
        clap_helpers::*,
        paths::{BINARY_SHORT, config_path, mm_cfg_path},
    },
    db::DbTable,
};
use fist_types::filetypes::FileTypeArg;
use fist_types::filters::{DbSortOrder, SortOrder, Visibility};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(disable_help_subcommand = true)]
#[command(disable_help_flag = true)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: SubCmd,
    #[command(flatten)]
    pub opts: CliOpts,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

impl Cli {
    pub fn parse_custom() -> Self {
        let first = std::env::args_os().nth(1);

        const SUBCMDS: &[&str] = &[
            ":open", ":o", ":app", ":a", ":file", ":dir", ":fd", "::", ":rg", ":", ":tool", ":t",
            ":info",
        ];

        let non_default = match first.as_deref().map(|s| s.to_str()) {
            None => false,
            Some(None) => true,
            Some(Some(arg)) => SUBCMDS.contains(&arg),
        };

        if non_default {
            return match Cli::try_parse() {
                Ok(cli) => cli,
                Err(e) => match e.kind() {
                    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                        e.print().expect("Failed to print help/version");
                        std::process::exit(0);
                    }
                    _ => e.exit(),
                },
            };
        }

        match NavCli::try_parse() {
            Ok(cli) => cli.into(),
            Err(err) => {
                dbg!(err);
                Cli::try_parse().unwrap_or_else(|e| e.exit())
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(disable_help_subcommand = true)]
#[command(disable_help_flag = true)]
pub struct NavCli {
    #[command(flatten)]
    pub args: DefaultCommand,
    #[command(flatten)]
    pub opts: CliOpts,
}

impl From<NavCli> for Cli {
    fn from(NavCli { args, opts }: NavCli) -> Self {
        Cli {
            subcommand: SubCmd::Fd(args),
            opts,
            help: (),
        }
    }
}

#[derive(Debug, Parser, Default, Clone)]
pub struct CliOpts {
    #[arg(long, global = true, default_value_t = 2)]
    pub verbosity: u8,

    /// config override
    #[arg(long = "override", global = true, value_name = "PATH")]
    pub config_override: Option<String>,
    /// config path
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        default_value_os_t = config_path().to_path_buf()
    )]
    pub config: PathBuf,
    /// matchmaker config path
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        default_value_os_t = mm_cfg_path().to_path_buf()
    )]
    pub mm_config: PathBuf,

    #[arg(
        long,
        help = r#"Dump the main config and any other missing configuration
files to default locations:
If the output was detected to have been redirected, this prints the main configuration.
Otherwise, this WILL OVERWRITE your main config."#
    )]
    pub dump_config: bool,

    #[arg(long, global = true, default_value_t)]
    pub style: ClapStyleSetting,
}

impl CliOpts {
    pub fn verbosity(&self) -> u8 {
        // (2 + self.verbose).saturating_sub(self.quiet)
        self.verbosity
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum SubCmd {
    #[command(name = ":open", visible_aliases = [":o"])]
    Open(OpenCmd),
    #[command(name = ":file")]
    Files(FilesCmd),
    #[command(name = ":dir")] // shell script wraps this with z
    Dirs(DirsCmd),
    /// Find and browse. (Default)
    #[command(name = ":fd", visible_aliases = ["::"])]
    Fd(DefaultCommand),
    #[command(name = ":rg", visible_aliases = [":"])]
    Rg(RgCommand),
    #[command(name =  ":tool", visible_aliases = [":t"])]
    Tools(ToolsCmd),
    #[command(name = ":info")]
    Info(InfoCmd),
}

/// Stats and database records
#[derive(Debug, Parser, Default, Clone)]
pub struct InfoCmd {
    /// history sort order.
    #[arg(long, default_value_t)]
    pub sort: DbSortOrder,
    /// history table to display.
    pub table: Option<DbTable>,
    #[arg(short, long)]
    /// maximum history entries to display.
    pub limit: Option<usize>,
    /// Don't print decorations.
    #[arg(short, long)]
    pub minimal: bool,
    // intro: slideshow showing some important keys in a table?
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

/// Launch apps and files
#[derive(Debug, Parser, Default, Clone)]
pub struct OpenCmd {
    /// app to open files with.
    #[clap(short = 'w', long, value_name = "PROG")]
    pub with: Option<OsString>,
    /// files to open.
    pub files: Vec<OsString>,

    #[arg(long)]
    pub list: bool,
    /// initial query.
    // #[arg(long, default_value_t)]
    // pub query: String,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

/// Recent folders
#[derive(Debug, Parser, Default, Clone)]
pub struct DirsCmd {
    /// history sort order.
    #[arg(long, default_value_t)]
    pub sort: DbSortOrder,

    #[arg(
        short,
        long,
        value_enum,
        num_args = 0..=1,
        default_missing_value = "_"
    )]
    pub list: Option<ListMode>,

    /// print the first match.
    #[arg(long)]
    pub cd: bool,
    /// initial query.
    #[arg(trailing_var_arg = true)]
    pub query: Vec<String>,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}
/// Recent files
#[derive(Debug, Parser, Default, Clone)]
pub struct FilesCmd {
    /// history sort order.
    #[arg(long, default_value_t)]
    pub sort: DbSortOrder,
    #[arg(
        short,
        long,
        value_enum,
        num_args = 0..=1,
        default_missing_value = "_"
    )]
    pub list: Option<ListMode>,

    /// initial query.
    #[arg(long, default_value_t)]
    pub query: String,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

/// Full text search
#[derive(Debug, Parser, Default, Clone)]
pub struct RgCommand {
    #[command(flatten)]
    pub vis: Visibility,
    pub sort: SortOrder,

    /// Files or directories to search in.
    #[arg(short = 'p', long = "path", value_name = "PATH")]
    pub paths: Vec<OsString>,

    /// Patterns to search (`rg -e`).
    #[arg(value_name = "PATTERNS")]
    pub patterns: Vec<String>,

    /// Args passed on verbatim to rg.
    #[arg(last = true, value_name = "RG_ARGS")]
    pub rg_args: Vec<OsString>,

    // top level rg args reexposed for convenience
    #[command(flatten)]
    pub case: CaseArgs,
    #[command(flatten)]
    pub context: ContextArgs,

    // /// initial query.
    // #[arg(long, default_value_t)]
    // pub query: String,

    // todo: wider support (override default print handler)
    /// Format the output as this template.
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(long)]
    pub list: bool,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

/// Browse listed files from standard input, the current directory, or fd.
#[derive(Debug, Parser, Default, Clone)]
#[command(
    override_usage = format!("{BINARY_SHORT} [OPTIONS] [PATHS]... [PATTERN] [-- <FD_ARGS>...]")
)]
pub struct DefaultCommand {
    #[arg(long)]
    pub sort: Option<SortOrder>,
    #[command(flatten)]
    pub vis: Visibility,

    /// print the first match.
    #[arg(long)]
    pub cd: bool,

    /// restrict search to certain file types and extensions
    /// (use `:t types` to list).
    #[arg(short = 't', long = "types", value_delimiter = ',')]
    pub types: Vec<FileTypeArg>,

    #[arg(value_name = "PATHS")]
    /// Paths to search in. Searches the current directory if none specified (and not otherwise configured).
    pub paths: Vec<OsString>,

    /// Args passed on verbatim to fd.
    #[arg(last = true, value_name = "FD_ARGS")]
    // its neat that this works: using -- passes to fd only if paths has entries which is exactly what we want
    pub fd: Vec<OsString>,

    /// Never stream input from stdin.
    #[arg(long, default_value_t)]
    pub no_read: bool,

    // todo: wider support (override default print handler)
    /// Format the output as this template.
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(long)]
    pub list: bool,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

/// Plugins and utilities
#[derive(Debug, Parser)]
pub struct ToolsCmd {
    #[command(subcommand)]
    pub tool: Option<SubTool>,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}
