use cba::broc::SHELL;
use std::process::Stdio;

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
