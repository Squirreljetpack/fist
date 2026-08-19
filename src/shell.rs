use cba::{broc::current_shell, prints};

use crate::cli::clap_tools::ShellCommand;

pub fn default_dir_widget_bind(shell: &str) -> &'static str {
    match shell {
        "fish" | "bash" => "\\e[1;2D",
        "nu" | "nushell" => "shift+left",
        _ => "^[[1;2D",
    }
}

pub fn default_file_widget_bind(shell: &str) -> &'static str {
    match shell {
        "fish" | "bash" => "\\e[1;2C",
        "nu" | "nushell" => "shift+right",
        _ => "^[[1;2C",
    }
}

pub fn default_rg_widget_bind(shell: &str) -> &'static str {
    match shell {
        "fish" | "bash" => "\\e[1;2B",
        "nu" | "nushell" => "shift+down",
        _ => "^[[1;2B",
    }
}

pub fn parse_nushell_key(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let parts: Vec<&str> = raw
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }

    let keycode = parts.last().unwrap().to_string();
    let mod_str = match parts.len() {
        1 => "none".to_string(),
        2 => parts[0].to_string(),
        _ => format!("[{}]", parts[..parts.len() - 1].join(" ")),
    };

    Some((mod_str, keycode))
}

fn generate_nushell_keybindings(
    dir_bind: Option<(String, String)>,
    file_bind: Option<(String, String)>,
    rg_bind: Option<(String, String)>,
) -> String {
    let mut entries = Vec::new();
    if let Some((m, k)) = dir_bind {
        entries.push(format!(
            r#"            {{
                name: fist_dir_widget
                modifier: {m}
                keycode: {k}
                mode: [emacs vi_insert vi_normal]
                event: {{
                    send: executehostcommand
                    cmd: "__fist_dir_widget"
                }}
            }}"#
        ));
    }
    if let Some((m, k)) = file_bind {
        entries.push(format!(
            r#"            {{
                name: fist_file_widget
                modifier: {m}
                keycode: {k}
                mode: [emacs vi_insert vi_normal]
                event: {{
                    send: executehostcommand
                    cmd: "__fist_file_widget"
                }}
            }}"#
        ));
    }
    if let Some((m, k)) = rg_bind {
        entries.push(format!(
            r#"            {{
                name: fist_rg_widget
                modifier: {m}
                keycode: {k}
                mode: [emacs vi_insert vi_normal]
                event: {{
                    send: executehostcommand
                    cmd: "__fist_rg_widget"
                }}
            }}"#
        ));
    }

    if entries.is_empty() {
        String::new()
    } else {
        format!(
            r#"export-env {{
    $env.config = (
        $env.config?
        | default {{}}
        | upsert keybindings {{ default [] }}
    )

    $env.config.keybindings = (
        $env.config.keybindings
        | append [
{}
        ]
    )
}}"#,
            entries.join("\n")
        )
    }
}

pub fn generate_shell(
    ShellCommand {
        z_name,
        z_dot_args,
        z_slash_args,
        z_dir_args,
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
        nav_name,
        shell,
    }: &ShellCommand,
    path: &str,
) -> String {
    let tag = shell.clone().unwrap_or_else(current_shell);
    let mut s = filter_by_tag(include_str!("../assets/shell/shell.zsh"), &tag);
    if *aliases {
        s.push_str("\n\n");
        s.push_str(&filter_by_tag(include_str!("../assets/shell/aliases.shrc"), &tag));
    }

    let dir_bind = dir_widget_bind
        .as_deref()
        .unwrap_or_else(|| default_dir_widget_bind(&tag));
    let file_bind = file_widget_bind
        .as_deref()
        .unwrap_or_else(|| default_file_widget_bind(&tag));
    let rg_bind = rg_widget_bind
        .as_deref()
        .unwrap_or_else(|| default_rg_widget_bind(&tag));

    let nu_keybindings = if tag == "nu" || tag == "nushell" {
        generate_nushell_keybindings(
            parse_nushell_key(dir_bind),
            parse_nushell_key(file_bind),
            parse_nushell_key(rg_bind),
        )
    } else {
        String::new()
    };

    s.replace("$${Z_NAME}", z_name)
        .replace("$${Z_DOT_ARGS}", z_dot_args)
        .replace("$${Z_SLASH_ARGS}", z_slash_args)
        .replace("$${Z_DIR_ARGS}", z_dir_args)
        //
        .replace("$${OPEN_NAME}", open_name)
        .replace("$${OPEN_CMD}", open_cmd)
        //
        .replace("$${DIRW_BIND}", dir_bind)
        .replace("$${FILEW_BIND}", file_bind)
        .replace("$${RGW_BIND}", rg_bind)
        //
        .replace("$${NU_KEYBINDINGS_BLOCK}", &nu_keybindings)
        //
        .replace(
            "$${FILEW_CMD}",
            file_open_cmd.as_ref().unwrap_or(open_cmd),
        )
        .replace("$${RGW_CMD}", rg_open_cmd.as_ref().unwrap_or(open_cmd))
        //
        .replace("$${DIRW_ARGS}", dir_widget_args)
        .replace("$${FILEW_ARGS}", file_widget_args)
        .replace("$${RGW_ARGS}", rg_widget_args)
        //
        .replace("$${NAV_NAME}", nav_name)
        .replace("$${BINARY_PATH}", path)
}

pub fn print_shell(cmd: &ShellCommand, path: &str) {
    let s = generate_shell(cmd, path);
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
    use std::{io::Write, process::Command};
    use clap::Parser;
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

    fn check_syntax(bin: &str, script: &str) {
        if !cba::broc::has(bin) {
            return;
        }
        let status = if bin == "nu" {
            Command::new(bin).arg("-c").arg(script).status()
        } else {
            let mut child = Command::new(bin)
                .arg("-n")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(script.as_bytes())
                .unwrap();
            child.wait()
        };
        assert!(status.unwrap().success(), "syntax check failed for {}", bin);
    }

    #[test]
    fn test_shell_syntax_and_generation() {
        for (shell, checker) in [
            ("zsh", "zsh"),
            ("bash", "bash"),
            ("fish", "fish"),
            ("nu", "nu"),
            ("nushell", "nu"),
            ("sh", "sh"),
            ("posix", "sh"),
            ("dash", "sh"),
        ] {
            let mut cmd = ShellCommand::parse_from(["fs :tool shell", "--aliases"]);
            cmd.shell = Some(String::from(shell));
            let script = generate_shell(&cmd, "/usr/bin/fs");
            assert!(!script.is_empty());
            check_syntax(checker, &script);
        }
    }

    #[test]
    fn test_default_widget_binds() {
        assert_eq!(default_dir_widget_bind("zsh"), "^[[1;2D");
        assert_eq!(default_dir_widget_bind("bash"), "\\e[1;2D");
        assert_eq!(default_dir_widget_bind("fish"), "\\e[1;2D");
        assert_eq!(default_dir_widget_bind("nu"), "shift+left");

        assert_eq!(default_file_widget_bind("zsh"), "^[[1;2C");
        assert_eq!(default_file_widget_bind("bash"), "\\e[1;2C");
        assert_eq!(default_file_widget_bind("fish"), "\\e[1;2C");
        assert_eq!(default_file_widget_bind("nu"), "shift+right");

        assert_eq!(default_rg_widget_bind("zsh"), "^[[1;2B");
        assert_eq!(default_rg_widget_bind("bash"), "\\e[1;2B");
        assert_eq!(default_rg_widget_bind("fish"), "\\e[1;2B");
        assert_eq!(default_rg_widget_bind("nu"), "shift+down");
    }

    #[test]
    fn test_parse_nushell_key() {
        assert_eq!(parse_nushell_key("shift+left"), Some(("shift".to_string(), "left".to_string())));
        assert_eq!(parse_nushell_key("control+shift+down"), Some(("[control shift]".to_string(), "down".to_string())));
        assert_eq!(parse_nushell_key("control+alt+shift+k"), Some(("[control alt shift]".to_string(), "k".to_string())));
        assert_eq!(parse_nushell_key("alt+enter"), Some(("alt".to_string(), "enter".to_string())));
        assert_eq!(parse_nushell_key("f1"), Some(("none".to_string(), "f1".to_string())));
        assert_eq!(parse_nushell_key(""), None);
        assert_eq!(parse_nushell_key("   "), None);
    }
}
