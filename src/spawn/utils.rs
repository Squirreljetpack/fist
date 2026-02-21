use cli_boilerplate_automation::broc::SHELL;

pub fn tokio_command_from_script(script: &str) -> tokio::process::Command {
    let (shell, arg) = &*SHELL;

    let mut ret = tokio::process::Command::new(shell);

    ret.arg(arg).arg(script).arg(""); //

    ret
}
