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
    use crate::cli::tool_types::*;

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
        Shell(ShellCommand),
        /// Context and preset dependent file handler
        Lessfilter(LessfilterCommand),
        /// Bump history entries
        Bump(BumpCommand),
        /// List mappings supported by the --type parameter.
        Types(TypesCommand),
    }
}
include!("build/completions_mock.rs");

// -----------------------------------------------------------------------------
// Include
// -----------------------------------------------------------------------------
include!("src/cli/clap.rs");

fn main() {
    println!("cargo:rerun-if-changed=src/cli/types.rs");
    println!("cargo:rerun-if-changed=build.rs");

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
