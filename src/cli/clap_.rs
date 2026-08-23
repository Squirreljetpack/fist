// split out so that build.rs can mock these subcommands with their actual structs
use clap::Subcommand;
use std::ffi::OsString;

#[derive(Subcommand, Debug, Clone, strum_macros::Display)]
// lowercase keeps the clap command names and the fuzzy picker's strum
// Display in lockstep (DiskSpace -> diskspace, ShowBinds -> showbinds)
#[command(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum SubTool {
    Colors,
    /// List directory (eza wrapper)
    #[command(alias = "lz")]
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
    #[command(alias = "lf")]
    Lessfilter {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    #[command(alias = "pg")]
    /// Page a file or stdin through bat into minus (single optional path).
    Pager {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Bump history entries
    Bump {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Trash files with timed fallback prompts.
    Trash {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Show binds.
    ShowBinds,
    /// List mappings supported by the --type parameter.
    Types {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Disk usage: compute directory sizes concurrently and print them.
    #[command(alias = "ds")]
    DiskSpace {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// Validate configuration and scripts; exits non-zero on errors.
    Check,
    /// Display an error message and wait for keypress.
    #[command(
        name = "showerror",
        alias = "show_error",
        alias = "show-error",
        alias = "se"
    )]
    #[strum(serialize = "showerror")]
    ShowError {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
}
