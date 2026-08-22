pub mod backend;
pub mod config;
pub mod lexer;

use std::ffi::OsString;

use crate::errors::CliError;
use lexer::parse_liza_args;

pub const LIZA_HELP: &str = include_str!("../../../assets/help/liza.txt");

pub fn handle(args: Vec<OsString>) -> Result<(), CliError> {
    let config = parse_liza_args(&args);

    if config.show_help {
        print!("{LIZA_HELP}");
        return Ok(());
    }

    if backend::eza::is_eza_available() {
        backend::eza::run(&config)
    } else {
        backend::native::run(&config)
    }
}
