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

/// macOS-specific binds added to mm.toml to produce assets/config/mac.mm.toml
/// (see `src/run/mm_config.rs`, which includes that file on macOS):
///
/// - `alt-backspace` is rewritten from `Trash` to `DeleteWord` — the two
///   platforms genuinely differ on this key, and TOML forbids redefining a
///   key, so the value is replaced in place;
/// - `ctrl-h` and `cmd-backspace` (trash on macOS) are inserted just before
///   the `# alternative if no delete key:` group.
const MAC_MM_BINDS: &str = "\
\"ctrl-h\" = \"Trash\"
\"cmd-backspace\" = \"Trash\"
";

fn generate_mac_mm_toml() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let src = manifest_dir.join("assets").join("config").join("mm.toml");
    let out = manifest_dir
        .join("assets")
        .join("config")
        .join("mac.mm.toml");
    let content = std::fs::read_to_string(&src).expect("read mm.toml");
    let content = content.replace(
        "\"alt-backspace\" = \"Trash\"",
        "\"alt-backspace\" = \"DeleteWord\"",
    );
    // insert just before the "alternative if no delete key" group
    let marker = "# alternative if no delete key:";
    let at = content
        .find(marker)
        .expect("mm.toml must contain the alt-key group");
    let mut inserted = String::with_capacity(content.len() + MAC_MM_BINDS.len() + 1);
    inserted.push_str(&content[..at]);
    inserted.push('\n');
    inserted.push_str(MAC_MM_BINDS);
    inserted.push_str(&content[at..]);
    std::fs::write(&out, inserted).expect("write mac.mm.toml");
    println!("cargo::rerun-if-changed=assets/config/mm.toml");
}

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=build/completions_mock.rs");
    println!("cargo::rerun-if-changed=src");

    generate_mac_mm_toml();

    let out_dir = {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = manifest_dir.join("assets").join("completions");
        std::fs::create_dir_all(&out_dir).unwrap();
        out_dir
    };

    let mut cmd = CliWithDefault::command();

    for shell in [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell] {
        generate_to(shell, &mut cmd, cli::paths::BINARY_SHORT, &out_dir).unwrap();
    }
}
