use std::process::Stdio;

use cba::{StringError, bait::ResultExt, broc::SHELL};
use tokio::process::{ChildStdout, Command};

pub fn tokio_command_from_script(script: &str) -> tokio::process::Command {
    let (shell, arg) = &*SHELL;

    let mut ret = tokio::process::Command::new(shell);

    ret.arg(arg).arg(script).arg(""); //

    ret
}

pub fn spawn_piped_tokio(cmd: &mut Command) -> Result<ChildStdout, StringError> {
    log::debug!("Spawning piped: {cmd:?}");

    match cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .prefix(format!("Failed to spawn: {:?}", cmd))?
        .stdout
        .take()
    {
        Some(s) => Ok(s),
        None => Err(format!("No stdout for {:?}.", cmd).into()),
    }
}
