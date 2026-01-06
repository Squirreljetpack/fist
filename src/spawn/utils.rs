use cli_boilerplate_automation::{
    _log,
    broc::{SHELL, tty_or_inherit},
};
use std::process::Command;

// modification to output in tty
pub fn exec_script_to_tty(
    script: &str,
    vars: impl IntoIterator<Item = (String, String)>,
) -> ! {
    let (shell, arg) = &*SHELL;

    let mut cmd = Command::new(shell);
    cmd.arg(arg).arg(script).envs(vars).stdout(tty_or_inherit());
    _log!("Spawning detached: {cmd:?}");

    #[cfg(not(windows))]
    {
        // replace current process
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        use std::process::exit;

        use cli_boilerplate_automation::ebog;

        ebog!("Could not exec {script:?}: {err}");
        exit(1);
    }

    #[cfg(windows)]
    {
        match command.status() {
            Ok(status) => {
                exit(
                    status
                        .code()
                        .unwrap_or(if status.success() { 0 } else { 1 }),
                );
            }
            Err(err) => {
                ebog!("Could not exec {script:?}: {err}");
                exit(1);
            }
        }
    }
}
