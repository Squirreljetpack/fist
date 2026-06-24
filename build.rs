use clap::CommandFactory;
use clap_complete::{Shell, generate_to};
use std::env;

mod cli {
    #![allow(unused)]
    mod tool_types {
        include!("src/cli/clap_tools.rs");
    }
    pub mod paths {
        include!("src/cli/paths.rs");
    }
    pub mod clap_helpers {
        include!("src/cli/clap_helpers.rs");
    }
    use crate::cli::tool_types::*;

    include!("src/cli/clap_.rs");
}
include!("build/completions_mock.rs");

// -----------------------------------------------------------------------------
// Include
// -----------------------------------------------------------------------------
include!("src/cli/clap.rs");

fn main() {
    let out_dir = {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = manifest_dir.join("assets").join("completions");
        std::fs::create_dir_all(&out_dir).unwrap();
        out_dir
    };

    let mut cmd = Cli::command();

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        generate_to(shell, &mut cmd, cli::paths::BINARY_SHORT, &out_dir).unwrap();
    }
}
