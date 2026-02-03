// split out so that build.rs can mock these subcommands with their actual structs
use clap::Subcommand;
use std::ffi::OsString;

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
    /// Show binds.
    ShowBinds,
    /// List mappings supported by the --type parameter.
    Types {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
}
