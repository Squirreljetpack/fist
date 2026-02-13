use cli_boilerplate_automation::{broc::current_shell, prints};

use crate::cli::clap_tools::ShellCommand;

pub fn print_shell(
    ShellCommand {
        z_name,
        z_dot_args,
        z_slash_args,
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
        shell,
    }: &ShellCommand,
    path: &str,
) {
    let mut s = include_str!("../assets/shell/shell.zsh")
        .replacen("$${Z_NAME}", z_name, 1)
        .replace("$${Z_DOT_ARGS}", z_dot_args)
        .replace("$${Z_SLASH_ARGS}", z_slash_args)
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

    let tag = shell.clone().unwrap_or_else(current_shell);
    s = filter_by_tag(&s, "zsh");

    prints!(s)
}

pub fn filter_by_tag(
    content: &str,
    tag: &str,
) -> String {
    let mut hide = false;
    let mut out = Vec::new();
    let matches = |after: &str| {
        let first_word = after.split_whitespace().next().unwrap_or("");
        first_word.split(',').any(|seg| seg == tag)
    };

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let (before, after_hash) = match line.find("#:") {
            Some(idx) => (&line[..idx], Some(&line[idx + 2..])),
            None => {
                // trim comments
                if let Some(idx) = line.find("# ") {
                    if line[..idx].trim().is_empty() {
                        continue;
                    }
                }
                (line, None)
            }
        };

        if let Some(after) = after_hash {
            // toggle directive: line begins with '#:'
            if before.trim().is_empty() {
                if after.is_empty() {
                    hide = false;
                    continue;
                }

                hide = !matches(after);
                continue;
            }

            if hide {
                continue;
            }

            if matches(after) {
                out.push(before.trim_end());
            }
        } else if !hide {
            out.push(line);
        }
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_blocks_and_keep_no_hash() {
        let input = "\
visible
#: foo
inside foo
interior #: foo
#:bar
hidden line
#:
  # trimmed
visible again
hidden #: bar
shown #: bar,foo
";

        let foo = "\
visible
inside foo
interior
visible again
shown";

        assert_eq!(filter_by_tag(input, "foo"), foo);

        let no_tag = "\
visible
visible again";
        assert_eq!(filter_by_tag(input, ""), no_tag);
    }
}
