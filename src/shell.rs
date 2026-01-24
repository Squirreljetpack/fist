use cli_boilerplate_automation::prints;

use crate::cli::clap_tools::ShellCommand;

pub fn print_shell(
    ShellCommand {
        z_name,
        zz_name,
        z_slash_name,
        visual,
        z_sort,
        z_dot_args,
        z_slash_args,
        aliases,
    }: &ShellCommand,
    path: &str,
) {
    let mut s = include_str!("../assets/shell/shell.zsh")
        .replacen("$${Z_NAME}", z_name, 1)
        .replacen("$${Z_SLASH_NAME}", z_slash_name, 1)
        .replace("$${Z_SORT}", z_sort.into())
        .replace("$${Z_DOT_ARGS}", z_dot_args)
        .replace("$${Z_SLASH_ARGS}", z_slash_args)
        .replacen("$${ZZ_NAME}", zz_name, 1)
        .replace("$${BINARY_PATH}", path)
        .replace("$${VISUAL}", visual);

    if *aliases {
        s.push_str("\n\n");
        s.push_str(include_str!("../assets/shell/aliases.shrc"));
    }

    prints!(s)
}
