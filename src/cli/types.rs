use std::{ffi::OsString, path::PathBuf};

use clap::{ArgAction, Parser, Subcommand, error::ErrorKind};

use crate::{
    cli::{BINARY_SHORT, tool_types::SubTool},
    db::{DbSortOrder, DbTable},
    filters::{SortOrder, Visibility},
    find::fd::FileTypeArg,
};

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
            ":open", ":o", ":app", ":a", ":file", ":dir", "::", ":", ":tool", ":t", ":info",
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
    /// + verbosity
    #[arg(short, global = true, action = ArgAction::Count)]
    pub verbose: u8,
    /// - verbosity
    #[arg(short, global = true, action = ArgAction::Count)]
    pub quiet: u8,
    /// config path
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
    /// matchmaker config path
    #[arg(long, global = true, value_name = "PATH")]
    pub mm_config: Option<PathBuf>,

    #[arg(
        long,
        help = r#"Dump the main config and any other missing configuration
files to standard locations.
Configs will be instead printed if stdout is redirected.
If not redirected, this WILL OVERWRITE your main config."#
    )]
    pub dump_config: bool,
}

impl CliOpts {
    pub fn verbosity(&self) -> u8 {
        (2 + self.verbose).saturating_sub(self.quiet)
    }
}

#[derive(Subcommand, Debug)]
pub enum SubCmd {
    #[command(name = ":open", visible_aliases = [":o"])]
    Open(OpenCmd),
    #[command(name = ":app", visible_aliases = [":a"])]
    Apps(AppsCmd),
    #[command(name = ":file")]
    Files(FilesCmd),
    #[command(name = ":dir")] // shell script wraps this with z
    Dirs(DirsCmd),
    /// Find and browse. (Default)
    #[command(name = "::")]
    Fd(DefaultCommand),
    #[command(name = ":")]
    Rg(RgCommand),
    #[command(name =  ":tool", visible_aliases = [":t"])]
    Tools(ToolsCmd),
    #[command(name = ":info")]
    Info(InfoCmd),
}

/// Open files by path
#[derive(Debug, Parser, Default, Clone)]
pub struct OpenCmd {
    /// app to open files with.
    #[arg(short = 'w', short_alias = 'a', long)]
    pub with: Option<OsString>,
    /// Positional arguments
    pub files: Vec<OsString>,

    #[arg(long, action = ArgAction::Help)]
    pub help: (),
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

/// App launcher
#[derive(Debug, Parser, Default, Clone)]
pub struct AppsCmd {
    #[clap(long, value_name = "PROG", num_args(0..=1))]
    with: Option<Option<String>>,

    /// files to open in app.
    pub files: Vec<OsString>,

    #[arg(long)]
    pub list: bool,
    /// initial query.
    #[arg(long, default_value_t)]
    pub query: String,
    #[arg(long, action = ArgAction::Help)]
    pub help: (),
}

/// Recent folders
#[derive(Debug, Parser, Default, Clone)]
pub struct DirsCmd {
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

    /// initial query.
    #[arg(long, default_value_t)]
    pub query: String,
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
    /// restrict search to certain file types and extensions
    /// (use `:t types` to list).
    #[arg(short = 't', long = "types", value_delimiter = ',')]
    pub types: Vec<FileTypeArg>,
    #[arg(value_name = "PATHS")]
    /// Paths to search in. (Default: ~)
    pub paths: Vec<OsString>,
    /// Args passed on verbatim to fd.
    #[arg(last = true, value_name = "FD_ARGS")]
    pub fd: Vec<OsString>,
    #[arg(long)]
    pub list: bool,
    /// print the first match.
    #[arg(long)]
    pub cd: bool,

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

// -----------------------------------------

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ListMode {
    #[value(name = "_")]
    Default,
    All,
}
