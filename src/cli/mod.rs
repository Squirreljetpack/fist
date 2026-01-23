pub mod handlers;
pub mod paths;
pub mod tool_types;
mod types;

pub use types::*;
pub mod env;
pub mod matchmaker;

pub static BINARY_FULL: &str = "fist";
pub static BINARY_SHORT: &str = "fs";

// -------------------------------
use std::ffi::OsString;

#[non_exhaustive]
#[derive(clap::Subcommand, Debug, Clone, strum_macros::Display)]
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

// use std::env;
// use clap::{error::ErrorKind, Parser};
// fn parse_cli_or_exit<T: Parser>(args: &[String]) -> T {
//     match T::try_parse_from(args) {
//         Ok(cli) => {
//             cli
//         },
//         Err(err) => match err.kind() {
//             ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
//                 err.print().expect("Failed to print help/version");
//                 std::process::exit(0);
//             }
//             _ => err.exit(),
//         },
//     }
// }
