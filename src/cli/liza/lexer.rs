use std::{ffi::OsString, path::PathBuf};

use super::config::{LizaConfig, ViewMode};

pub fn parse_liza_args(args: &[OsString]) -> LizaConfig {
    let mut config = LizaConfig::default();
    let mut skip = false;

    // Track column intersections if multiple long-mode presets are requested
    // (matches the i_nl exclusion logic in the original liza script)
    let mut selected_columns = Vec::new();

    for arg in args {
        if skip {
            config.paths.push(PathBuf::from(arg));
            continue;
        }

        let s = arg.to_string_lossy();

        if let Some(mode_str) = s.strip_prefix("::") {
            if mode_str.is_empty() || mode_str == "h" || mode_str == "help" {
                config.show_help = true;
            } else {
                match mode_str {
                    "n" | "nav" => config.view_mode = Some(ViewMode::Nav),
                    "g" | "git" => config.view_mode = Some(ViewMode::Git),
                    "d" | "dir" | "dirs" => config.view_mode = Some(ViewMode::Dirs),
                    "f" | "flatten" => config.view_mode = Some(ViewMode::Flatten),
                    "t" | "tree" => config.view_mode = Some(ViewMode::Tree),
                    "r" | "recent" => config.view_mode = Some(ViewMode::Recent),
                    _ => config.show_help = true,
                }
            }
        } else if s.starts_with(':') && s.len() > 1 {
            for c in s[1..].chars() {
                match c {
                    'a' => {
                        config.all = true;
                    }
                    'b' => {
                        config.show_octal = true;
                        selected_columns.push("b");
                    }
                    't' => {
                        config.show_time = true;
                        config.header = true;
                        selected_columns.push("t");
                    }
                    'm' => {
                        config.show_mtime = true;
                        config.header = true;
                        selected_columns.push("m");
                    }
                    'l' => {
                        config.show_clean_long = true;
                        config.all = true;
                        config.header = true;
                        selected_columns.push("l");
                    }
                    'x' => {
                        config.show_extensive = true;
                        config.all = true;
                        config.header = true;
                        selected_columns.push("x");
                    }
                    'u' => {
                        config.pretty = true;
                    }
                    'T' => {
                        config.unbounded_tree = true;
                    }
                    '0'..='9' => {
                        if let Some(digit) = c.to_digit(10) {
                            config.tree_depth = Some(digit as usize);
                        }
                    }
                    'h' => {
                        config.header = true;
                    }
                    'i' => {
                        config.git_ignore = true;
                    }
                    's' => {
                        config.show_size = true;
                        selected_columns.push("s");
                    }
                    'V' => {
                        config.verbose = true;
                    }
                    'g' => {
                        config.git_status = true;
                        selected_columns.push("g");
                    }
                    _ => {
                        config.show_help = true;
                    }
                }
            }
        } else if s == "--no-header" {
            config.no_header = true;
            config.header = false;
        } else if s == "--help" || s == "-h" {
            config.show_help = true;
        } else if s == "-1" {
            config.one_line = true;
        } else if s == "--" {
            skip = true;
        } else if !s.starts_with('-') && config.passthrough_args.is_empty() {
            // First bare word when no passthrough flags exist starts paths
            config.paths.push(PathBuf::from(arg));
            skip = true;
        } else if s.starts_with('-') {
            config.passthrough_args.push(arg.clone());
        } else {
            config.paths.push(PathBuf::from(arg));
        }
    }

    // Resolve column exclusions for eza long mode
    apply_column_exclusions(&mut config, &selected_columns);

    config
}

fn apply_column_exclusions(
    config: &mut LizaConfig,
    columns: &[&'static str],
) {
    if columns.is_empty() {
        return;
    }

    // Default negation exclusions if any long columns are active
    let mut no_filesize = true;
    let mut no_user = true;
    let mut no_permissions = true;
    let mut no_time = true;

    for col in columns {
        match *col {
            "b" => {
                // keeps permissions & time
                no_permissions = false;
                no_time = false;
            }
            "t" | "m" => {
                // keeps time
                no_time = false;
            }
            "s" => {
                // keeps filesize
                no_filesize = false;
            }
            "l" | "x" => {
                // cleaner/extensive long includes all fields
                no_filesize = false;
                no_user = false;
                no_permissions = false;
                no_time = false;
            }
            "g" => {
                // git status column
            }
            _ => {}
        }
    }

    config.no_filesize = no_filesize;
    config.no_user = no_user;
    config.no_permissions = no_permissions;
    config.no_time = no_time;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_presets() {
        let args = vec![OsString::from(":al3u"), OsString::from("src")];
        let cfg = parse_liza_args(&args);
        assert!(cfg.all);
        assert!(cfg.show_clean_long);
        assert_eq!(cfg.tree_depth, Some(3));
        assert!(cfg.pretty);
        assert_eq!(cfg.paths, vec![PathBuf::from("src")]);
    }

    #[test]
    fn test_parse_view_mode() {
        let args = vec![OsString::from("::nav"), OsString::from("assets")];
        let cfg = parse_liza_args(&args);
        assert_eq!(cfg.view_mode, Some(ViewMode::Nav));
        assert_eq!(cfg.paths, vec![PathBuf::from("assets")]);
    }

    #[test]
    fn test_parse_passthrough_and_delimiter() {
        let args = vec![
            OsString::from(":a"),
            OsString::from("--sort=size"),
            OsString::from("-F"),
            OsString::from("--"),
            OsString::from("target"),
        ];
        let cfg = parse_liza_args(&args);
        assert!(cfg.all);
        assert_eq!(
            cfg.passthrough_args,
            vec![OsString::from("--sort=size"), OsString::from("-F")]
        );
        assert_eq!(cfg.paths, vec![PathBuf::from("target")]);
    }

    #[test]
    fn test_parse_sba_and_header() {
        let args = vec![OsString::from(":sbah")];
        let cfg = parse_liza_args(&args);
        assert!(cfg.all);
        assert!(cfg.show_size);
        assert!(cfg.show_octal);
        assert!(cfg.header);
        assert!(!cfg.no_filesize);
        assert!(!cfg.no_permissions);
        assert!(!cfg.no_time);
    }
}
