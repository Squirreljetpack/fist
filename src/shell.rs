use cli_boilerplate_automation::prints;

use crate::cli::tool_types::ShellCommand;

pub fn print_shell(
    ShellCommand {
        z_name,
        zz_name,
        visual,
        z_sort,
        z_dot_args,
        aliases,
    }: &ShellCommand,
    path: &str,
) {
    let mut s = include_str!("../assets/shell/shell.zsh")
        .replacen("$${Z_NAME}", z_name, 1)
        .replace("$${Z_SORT}", z_sort.into())
        .replace("$${Z_DOT_ARGS}", z_dot_args)
        .replacen("$${ZZ_NAME}", zz_name, 1)
        .replace("$${BINARY_PATH}", path)
        .replace("$${VISUAL}", visual);

    if *aliases {
        s.push_str("\n\n");
        s.push_str(include_str!("../assets/shell/aliases.shrc"));
    }

    prints!(s)
}
