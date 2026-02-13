mod program;
use std::ffi::OsString;
use std::sync::RwLock;
pub mod menu_action;
pub mod utils;

use crate::abspath::AbsPath;
use cli_boilerplate_automation::bait::TransformExt;
use cli_boilerplate_automation::vec_;
use cli_boilerplate_automation::{bog::BogOkExt, broc::ChildExt};
pub use program::*;
// -----------
use crate::db::{Connection, DbTable};
use crate::errors::CliError;

/// # note
/// Relative paths are resolved relative to the initial cwd
pub async fn open_wrapped(
    mut conn: Connection,
    prog: Option<Program>,
    files: &[OsString],
    // true when not executing or when opening app
    detach: bool,
) -> Result<(), CliError> {
    // todo: better errors
    open(prog.as_ref(), files)
        .ok_or(CliError::Handled)?
        .transform(|mut c| {
            if detach {
                c.spawn_detached().map(|_| {})
            } else {
                c._spawn()
                    .and_then(|mut child| (child.wait_for_code() == 0).then_some(()))
            }
        })
        .ok_or(CliError::Handled)?;

    if let Some(prog) = prog {
        let path = prog.path();
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

pub async fn bump_after_open(
    mut conn: Connection,
    prog: Option<Program>,
    files: &[OsString],
) {
    if let Some(prog) = prog {
        let path = prog.path();
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
}

use cli_boilerplate_automation::broc::{CommandExt, format_sh_command};
use std::process::Command;

/// Open some files, optionally with a [`Program`]
pub fn open(
    prog: Option<&Program>,
    files: &[OsString],
) -> Option<Command> {
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

    (!words.is_empty()).then(|| spawn(&words))
}

static SPAWN_WITH: RwLock<Vec<String>> = RwLock::new(Vec::new());

pub fn init_spawn_with(cmd: Vec<String>) {
    let mut guard = SPAWN_WITH.write().unwrap();
    *guard = cmd;
}

/// Requires words is nonempty
/// submit words for shell/spawn_with to execute
pub fn spawn(words: &[OsString]) -> Command {
    if let Ok(with) = SPAWN_WITH.read().as_ref()
        && !with.is_empty()
    {
        let script = format_sh_command(words, false);
        let program = &with[0];
        let args = &with[1..];

        let ok = Command::new(program).arg("status").success();
        if ok {
            return Command::new(program).with_args(args).with_arg(script);
        }
    }

    cfg_if::cfg_if! {
        if #[cfg(target_os = "windows")] {
            // todo: dunno how to format the script
            Command::new(words[0].clone())
            .with_args(&words[1..])
        } else {
            Command::new(words[0].clone())
            .with_args(&words[1..])
        }
    }
}
