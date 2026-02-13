use cli_boilerplate_automation::prints;

use crate::cli::clap_tools::ShellCommand;

pub fn print_shell(
    ShellCommand {
        z_name,
        z_dot_args,
        z_sort,
        open_name,
        open_cmd,
        dir_widget_bind,
        file_widget_bind,
        rg_widget_bind,
        file_open_cmd,
        rg_open_cmd,
        dir_widget_args,
        file_widget_args,
        rg_widget_args,
        aliases,
    }: &ShellCommand,
    path: &str,
) {
    let mut s = include_str!("../assets/shell/shell.zsh")
        .replacen("$${Z_NAME}", z_name, 1)
        .replace("$${Z_DOT_ARGS}", z_dot_args)
        .replace("$${Z_SORT}", z_sort.into())
        //
        .replacen("$${OPEN_NAME}", open_name, 1)
        .replace("$${OPEN_CMD}", open_cmd)
        //
        .replacen("$${DIRW_BIND}", dir_widget_bind, 1)
        .replacen("$${FILEW_BIND}", file_widget_bind, 1)
        .replacen("$${RGW_BIND}", rg_widget_bind, 1)
        //
        .replacen(
            "$${FILEW_CMD}",
            file_open_cmd.as_ref().unwrap_or(open_cmd),
            1,
        )
        .replacen("$${RGW_CMD}", rg_open_cmd.as_ref().unwrap_or(open_cmd), 1)
        //
        //
        .replacen("$${DIRW_ARGS}", dir_widget_args, 1)
        .replacen("$${FILEW_ARGS}", file_widget_args, 1)
        .replacen("$${RGW_ARGS}", rg_widget_args, 1)
        //
        .replace("$${BINARY_PATH}", path);

    if *aliases {
        s.push_str("\n\n");
        s.push_str(include_str!("../assets/shell/aliases.shrc"));
    }

    prints!(s)
}
