mod program;
use std::ffi::OsString;
use std::sync::Mutex;
pub mod utils;

use crate::abspath::AbsPath;
use cba::vec_;
use cba::{bog::BogOkExt, broc::ChildExt};
pub use program::*;
// -----------
use crate::db::{Connection, DbTable};
use crate::errors::CliError;
use crate::utils::command::command_log_stderr;

/// Open some files, then record the visit through `conn` when one is given.
///
/// # note
/// Relative paths are resolved relative to the initial cwd.
/// detach is usually specified iff fs is not exiting afterward.
/// todo: lowpri: option to silence stdout even when !detach?
pub async fn open_wrapped(
    conn: Option<Connection>,
    prog: Option<Program>,
    files: &[OsString],
    detach: bool,
) -> Result<(), CliError> {
    let cmds = open(prog.as_ref(), files);
    if cmds.is_empty() {
        return Err(CliError::Handled);
    }
    for mut c in cmds {
        let ok = if detach {
            c.spawn_detached().is_some()
        } else {
            // `-v` leaves the child's stderr on the terminal for debugging
            if let Some(stderr) = command_log_stderr() {
                c.stderr(stderr);
            }
            c._spawn()
                .is_some_and(|mut child| child.wait_for_code() == 0)
        };
        if !ok {
            return Err(CliError::Handled);
        }
    }

    if let Some(conn) = conn {
        bump_after_open(conn, prog, files).await;
    }
    Ok(())
}

/// Record an open in the history database: a program bumps the apps table,
/// plain files/folders go into history.
pub async fn bump_after_open(
    mut conn: Connection,
    prog: Option<Program>,
    files: &[OsString],
) {
    if let Some(prog) = prog {
        let path = prog.path();
        conn.switch_table(DbTable::apps);
        conn.bump_path(path, 1).await._wbog_(format!(
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

use cba::broc::{CommandExt, format_sh_command};
use std::process::Command;

/// Open some files, optionally with a [`Program`]
pub fn open(
    prog: Option<&Program>,
    files: &[OsString],
) -> Vec<Command> {
    // Build the command words to spawn
    let words: Vec<OsString> = if let Some(prog) = prog {
        let Some(mut cmd) = prog.to_cmd()._ebog() else {
            return Vec::new();
        };

        // append file arguments if any
        if !files.is_empty() {
            cmd.extend_from_slice(files);
        }
        cmd
    } else {
        // No program specified, just open files with default system behavior
        if cfg!(target_os = "macos") {
            let mut cmd = vec_![: "open"];
            cmd.extend_from_slice(files);
            cmd
        } else if cfg!(target_os = "linux") {
            let mut cmd = vec_![: "xdg-open"];
            cmd.extend_from_slice(files);
            cmd
        } else if cfg!(target_os = "windows") {
            // `start` treats a quoted first argument as the window title, so
            // paths with spaces need an explicit empty title. `start` only
            // opens one file per invocation, so spawn one command per file.
            return files
                .iter()
                .map(|f| spawn(&vec_![: "cmd", "/C", "start", "", f.clone()][..]))
                .collect();
        } else {
            eprintln!(
                "Unsupported platform, cannot construct command for files: {}",
                files
                    .iter()
                    .map(|f| f.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            return Vec::new();
        }
    };

    if words.is_empty() {
        Vec::new()
    } else {
        vec![spawn(&words)]
    }
}

/// Reveal `path` in the system file manager, without waiting for it.
///
/// Finder and Explorer select the item itself; other platforms open the
/// directory containing it. The root has no containing directory, so it is
/// opened directly.
pub fn reveal(path: &AbsPath) -> Result<(), CliError> {
    let has_parent = path.parent().is_some();

    let words: Vec<OsString> = if cfg!(target_os = "macos") {
        if has_parent {
            vec_![: "open", "-R", path]
        } else {
            vec_![: "open", path]
        }
    } else if cfg!(target_os = "windows") {
        // todo: untested
        if has_parent {
            let mut select = OsString::from("/select,");
            select.push(path.as_path());
            vec_![: "explorer", select]
        } else {
            vec_![: "explorer", path]
        }
    } else if cfg!(target_os = "linux") {
        vec_![: "xdg-open", path._parent()]
    } else {
        eprintln!("Unsupported platform, cannot reveal: {path}");
        return Err(CliError::Handled);
    };

    spawn(&words)
        .spawn_detached()
        .map(|_| ())
        .ok_or(CliError::Handled)
}

static SPAWN_WITH: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn init_spawn_with(cmd: Vec<String>) {
    let mut guard = SPAWN_WITH.lock().unwrap();
    *guard = cmd;
}

/// Requires words is nonempty
/// submit words for shell/spawn_with to execute
pub fn spawn(words: &[OsString]) -> Command {
    if let Ok(with) = SPAWN_WITH.lock().as_ref()
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
