use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};

use cba::broc::SHELL;

/// [`tokio::process::Command`] counterpart of `cba`'s `Command::from_script`,
/// always using the default [`SHELL`] (fist has no configurable shell).
pub fn tokio_from_script(script: &str) -> tokio::process::Command {
    let (def_sh, def_arg) = &*SHELL;

    let mut cmd = tokio::process::Command::new(def_sh);
    cmd.arg(def_arg);
    cmd.arg(script);

    #[cfg(unix)]
    cmd.arg("");

    cmd
}

pub fn maybe_tty() -> Stdio {
    if let Ok(mut tty) = std::fs::File::open("/dev/tty") {
        let _ = std::io::Write::flush(&mut tty); // does nothing but seems logical
        Stdio::from(tty)
    } else {
        log::error!("Failed to open /dev/tty");
        Stdio::inherit()
    }
}

/// Path of the stderr log for spawned commands, stored by `init_logger`.
/// Unset when spawned commands keep the terminal's stderr (`-v`).
pub static COMMAND_LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

static COMMAND_LOG: OnceLock<Option<File>> = OnceLock::new();

/// Stderr sink for a spawned command: the command log, opened (truncating the
/// file once per process) from [`COMMAND_LOG_PATH`], or `None` when the log is
/// not in use.
pub fn command_log_stderr() -> Option<Stdio> {
    let file = COMMAND_LOG.get_or_init(|| {
        let path = COMMAND_LOG_PATH.lock().unwrap();
        let path = path.as_ref()?;
        match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
        {
            Ok(file) => Some(file),
            Err(e) => {
                log::error!("Failed to open command log {}: {e}", path.display());
                None
            }
        }
    });
    file.as_ref()
        .and_then(|f| f.try_clone().ok())
        .map(Stdio::from)
}
