pub mod config;
pub mod handlers;
pub mod paths;
pub mod tool_types;
mod types;
pub use types::*;
pub mod env;
pub mod matchmaker;

pub static BINARY_FULL: &str = "fist";
pub static BINARY_SHORT: &str = "fs";

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
