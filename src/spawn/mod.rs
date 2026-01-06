mod program;
use std::ffi::OsString;
pub mod utils;

use crate::abspath::AbsPath;
use cli_boilerplate_automation::bog::BogOkExt;
use cli_boilerplate_automation::vec_;
pub use program::*;
// -----------
use crate::db::{Connection, DbTable};
use crate::errors::CliError;

pub async fn open_wrapped(
    mut conn: Connection,
    prog: Option<Program>,
    files: &[OsString],
) -> Result<(), CliError> {
    if open(prog.as_ref(), files).is_none() {
        return Err(CliError::Handled);
    };

    if let Some(prog) = prog {
        let path = &prog.path();
        conn.switch_table(DbTable::apps);
        conn.bump(path, 1).await._wbog_(format!(
            "Failed to record {}",
            prog.path().to_string_lossy()
        ));
    }

    if !files.is_empty() {
        conn.push_files_and_folders(files.iter().map(AbsPath::new))
            .await
            ._wbog_("Failed to record files");
    }

    Ok(())
}

use cfg_if::cfg_if;
use cli_boilerplate_automation::broc::{CommandExt, format_sh_command, has};
use std::process::{Child, Command};

/// Open some files, optionally with a [`Program`]
pub fn open(
    prog: Option<&Program>,
    files: &[OsString],
) -> Option<Child> {
    // Build the command words to spawn
    let words: Vec<OsString> = if let Some(prog) = prog {
        let mut cmd = prog.to_cmd()._ebog()?;

        // append file arguments if any
        if !files.is_empty() {
            cmd.extend_from_slice(files);
        }
        cmd
    } else {
        // No program specified, just open files with default system behavior
        if cfg!(target_os = "macos") {
            let mut cmd = vec_!["open"];
            cmd.extend_from_slice(files);
            cmd
        } else if cfg!(target_os = "linux") {
            let mut cmd = vec_!["xdg-open"];
            cmd.extend_from_slice(files);
            cmd
        } else if cfg!(target_os = "windows") {
            // todo: untested
            let mut cmd = vec_!["cmd", "/C", "start"];
            for f in files {
                cmd.push(f.clone());
            }
            cmd
        } else {
            eprintln!(
                "Unsupported platform, cannot construct command for files: {}",
                files
                    .iter()
                    .map(|f| f.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            return None;
        }
    };

    spawn(&words)
}

/// submit words for shell to execute
pub fn spawn(words: &[OsString]) -> Option<Child> {
    if words.is_empty() {
        return None;
    }
    if has("pueue") {
        // create the script for pueue to eval
        let script = format_sh_command(words);

        let args = vec!["add".into(), "--".into(), script];
        let ret = Command::new("pueue").args(args).spawn_detached();

        let pueue_ok = std::process::Command::new("pueue").arg("status").success();

        if pueue_ok {
            return ret;
        }
        // if pueued is down, retry
    }
    cfg_if! {
        if #[cfg(target_os = "windows")] {
            script = "";
            // todo: dunno how to format the script
            cli_boilerplate_automation::broc::spawn_script(script, vars, Stdio::null(), Stdio::null(), Stdio::null())
        } else {
            // spawn the program directly
            Command::new(words[0].clone()).args(&words[1..]).spawn_detached()
        }
    }
}
