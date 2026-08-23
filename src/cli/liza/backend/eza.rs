use ignore::WalkBuilder;
use std::{
    ffi::OsString,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use super::super::config::{LizaConfig, ViewMode};
use crate::{
    errors::CliError,
    pager::page_child,
    utils::tree::{TreeNode, render_tree},
};

pub fn is_eza_available() -> bool {
    Command::new("eza")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn run(config: &LizaConfig) -> Result<(), CliError> {
    if config.view_mode == Some(ViewMode::Git) {
        return run_git_view(config);
    }

    let mut args: Vec<OsString> = Vec::new();

    // Default base flags
    args.push("--smart-group".into());
    args.push("--time-style=iso".into());
    args.push("--color=always".into());

    // Basic filters
    if config.all {
        args.push("-a".into());
        args.push("--group-directories-first".into());
    }
    if config.git_ignore {
        args.push("--git-ignore".into());
    }
    if config.pretty {
        args.push("--icons=always".into());
    }
    if config.one_line {
        args.push("-1".into());
    }

    // Tree options
    if config.view_mode != Some(ViewMode::Flatten) {
        if config.unbounded_tree {
            args.push("-T".into());
        } else if let Some(depth) = config.tree_depth {
            args.push("-T".into());
            args.push("-L".into());
            args.push(depth.to_string().into());
        }
    }

    // View modes
    let mut is_paged_view = false;
    match &config.view_mode {
        Some(ViewMode::Nav) => {
            is_paged_view = true;
            let depth = std::env::var("MAX_DEPTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3);
            args.push("-T".into());
            args.push("--icons=always".into());
            args.push("-l".into());
            args.push("--no-permissions".into());
            args.push("--no-user".into());
            args.push("--no-filesize".into());
            args.push("--no-time".into());
            args.push("-L".into());
            args.push(depth.to_string().into());
        }
        Some(ViewMode::Dirs) => {
            is_paged_view = true;
            args.push("-T".into());
            args.push("--only-dirs".into());
            args.push("--total-size".into());
            args.push("--no-permissions".into());
            args.push("--no-user".into());
        }
        Some(ViewMode::Tree) => {
            is_paged_view = true;
            args.push("-T".into());
        }
        Some(ViewMode::Recent) => {
            args.push("--sort=accessed".into());
            args.push("--reverse".into());
            args.push("-1".into());
        }
        _ => {}
    }

    // Columns and long form
    let is_long = config.show_clean_long
        || config.show_extensive
        || config.show_octal
        || config.show_time
        || config.show_mtime
        || config.show_size
        || config.git_status
        || config.header;

    if is_long {
        args.push("-l".into());

        if config.show_clean_long {
            args.push("--sort=ext".into());
            args.push("-n@".into());
            args.push("--group".into());
        } else if config.show_extensive {
            args.push("--sort=ext".into());
            args.push("-n@".into());
            args.push("-ZuUmigO".into());
        }

        if config.show_octal {
            args.push("--no-permissions".into());
            args.push("--octal-permissions".into());
        }
        if config.show_time {
            args.push("-Umu".into());
            args.push("--sort=modified".into());
        }
        if config.show_mtime {
            args.push("--time=modified".into());
            args.push("--sort=modified".into());
            args.push("-r".into());
        }
        if config.git_status {
            let in_git = Command::new("git")
                .args(["rev-parse", "--git-dir"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            if in_git {
                args.push("--git".into());
            } else {
                args.push("--git-repos".into());
            }
        }

        // Apply long form column exclusions
        if config.no_permissions && !config.show_octal {
            args.push("--no-permissions".into());
        }
        if config.no_filesize {
            args.push("--no-filesize".into());
        }
        if config.no_user {
            args.push("--no-user".into());
        }
        if config.no_time {
            args.push("--no-time".into());
        }

        if config.header && !config.no_header {
            args.push("--header".into());
        }
    }

    // Append raw passthrough arguments
    for p in &config.passthrough_args {
        args.push(p.clone());
    }

    // Resolve paths (handling flatten mode if applicable)
    let paths: Vec<PathBuf> = if config.view_mode == Some(ViewMode::Flatten) {
        let targets = if config.paths.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            config.paths.clone()
        };

        let max_depth = if config.unbounded_tree {
            usize::MAX
        } else {
            config.tree_depth.unwrap_or(1)
        };

        flatten_directory_targets(&targets, max_depth, config)
    } else {
        config.paths.clone()
    };

    if !paths.is_empty() {
        args.push("--".into());
        for path in paths {
            args.push(path.into());
        }
    }

    if config.verbose {
        eprintln!(
            "eza {}",
            args.iter()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );
    }

    // Paging via fist pager if [ -t 1 ]
    let should_page = is_paged_view && std::io::stdout().is_terminal();

    if should_page {
        let child = Command::new("eza")
            .args(&args)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(CliError::IoError)?;

        page_child(child, None).map_err(CliError::IoError)?;
        Ok(())
    } else {
        let status = Command::new("eza")
            .args(&args)
            .status()
            .map_err(CliError::IoError)?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}

struct FlattenEntry {
    path: PathBuf,
    is_dir: bool,
    depth: usize,
}

pub fn flatten_directory_targets(
    targets: &[PathBuf],
    max_depth: usize,
    config: &LizaConfig,
) -> Vec<PathBuf> {
    let mut entries = Vec::new();

    for target in targets {
        if target.is_dir() {
            let mut builder = WalkBuilder::new(target);
            builder
                .max_depth(if max_depth == usize::MAX {
                    None
                } else {
                    Some(max_depth)
                })
                .hidden(!config.all)
                .git_ignore(config.git_ignore)
                .git_global(config.git_ignore)
                .git_exclude(config.git_ignore)
                .require_git(false);

            for result in builder.build() {
                match result {
                    Ok(entry) => {
                        if entry.depth() == 0 {
                            continue;
                        }
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        if is_dir && entry.depth() < max_depth {
                            continue;
                        }
                        let p = entry.path();
                        let clean_p = p
                            .strip_prefix("./")
                            .or_else(|_| p.strip_prefix("."))
                            .unwrap_or(p);
                        entries.push(FlattenEntry {
                            path: clean_p.to_path_buf(),
                            is_dir,
                            depth: entry.depth(),
                        });
                    }
                    Err(err) => eprintln!("'{}': {}", target.display(), err),
                }
            }
        } else {
            let clean_p = target
                .strip_prefix("./")
                .or_else(|_| target.strip_prefix("."))
                .unwrap_or(target);
            entries.push(FlattenEntry {
                path: clean_p.to_path_buf(),
                is_dir: false,
                depth: 0,
            });
        }
    }

    // Sort: files first (is_dir: false), then directories (is_dir: true).
    // Within each, sort by depth ascending, then alphabetically by path.
    entries.sort_by(|a, b| {
        a.is_dir
            .cmp(&b.is_dir)
            .then_with(|| a.depth.cmp(&b.depth))
            .then_with(|| a.path.cmp(&b.path))
    });

    // Deduplicate identical paths preserving sorted order
    entries.dedup_by(|a, b| a.path == b.path);

    entries.into_iter().map(|e| e.path).collect()
}

pub fn build_path_tree(paths: &[String]) -> Vec<TreeNode<String>> {
    let mut root_children: Vec<TreeNode<String>> = Vec::new();

    for path_str in paths {
        let path = Path::new(path_str);
        let components: Vec<_> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();
        if components.is_empty() {
            continue;
        }

        insert_components(&mut root_children, &components);
    }

    root_children
}

fn insert_components(
    nodes: &mut Vec<TreeNode<String>>,
    components: &[String],
) {
    if components.is_empty() {
        return;
    }

    let head = &components[0];
    let tail = &components[1..];

    if let Some(pos) = nodes.iter().position(|n| &n.value == head) {
        insert_components(&mut nodes[pos].children, tail);
    } else {
        let mut new_node = TreeNode::new(head.clone());
        insert_components(&mut new_node.children, tail);
        nodes.push(new_node);
    }
}

fn run_git_view(config: &LizaConfig) -> Result<(), CliError> {
    let target = config
        .paths
        .first()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "HEAD".to_string());

    let mut git_cmd = Command::new("git");
    git_cmd.args(["ls-tree", "-r", "--name-only", &target]);

    if config.verbose {
        eprintln!("git ls-tree -r --name-only {target}");
    }

    let output = git_cmd.output().map_err(CliError::IoError)?;
    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    let raw_text = String::from_utf8_lossy(&output.stdout);
    let files: Vec<String> = raw_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();

    let root_nodes = build_path_tree(&files);
    let mut tree_buf = Vec::new();
    render_tree(&root_nodes, &mut tree_buf).map_err(CliError::IoError)?;
    let rendered = String::from_utf8_lossy(&tree_buf);

    print!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_flatten_directory_targets_multi_depth() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        // Create structure:
        // root/c (file)
        // root/a (dir)
        // root/a/x (file)
        // root/a/y (dir)
        // root/a/y/z (file)
        // root/.hidden (file)
        File::create(root.join("c")).unwrap();
        fs::create_dir(root.join("a")).unwrap();
        File::create(root.join("a").join("x")).unwrap();
        fs::create_dir(root.join("a").join("y")).unwrap();
        File::create(root.join("a").join("y").join("z")).unwrap();
        File::create(root.join(".hidden")).unwrap();

        let mut config = LizaConfig::default();

        // 1. Flatten depth 1 without config.all (skips .hidden)
        let flattened_d1 = flatten_directory_targets(&[root.to_path_buf()], 1, &config);
        let names_d1: Vec<String> = flattened_d1
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names_d1, vec!["c".to_string(), "a".to_string()]);

        // 2. Flatten depth 1 with config.all (includes .hidden)
        config.all = true;
        let flattened_d1_all = flatten_directory_targets(&[root.to_path_buf()], 1, &config);
        let names_d1_all: Vec<String> = flattened_d1_all
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names_d1_all,
            vec![".hidden".to_string(), "c".to_string(), "a".to_string()]
        );

        config.all = false;

        // 3. Flatten depth 2 from root -> files first (c, a/x), then bottom-level dirs (a/y, skipping a)
        let flattened_d2 = flatten_directory_targets(&[root.to_path_buf()], 2, &config);
        let names_d2: Vec<String> = flattened_d2
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names_d2,
            vec!["c".to_string(), "a/x".to_string(), "a/y".to_string()]
        );

        // 4. Flatten depth 3 from root -> files first (c, a/x, a/y/z), intermediate dirs skipped
        let flattened_d3 = flatten_directory_targets(&[root.to_path_buf()], 3, &config);
        let names_d3: Vec<String> = flattened_d3
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            names_d3,
            vec!["c".to_string(), "a/x".to_string(), "a/y/z".to_string()]
        );
    }

    #[test]
    fn test_flatten_directory_targets_git_ignore() {
        use std::io::Write;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        let gitignore = root.join(".gitignore");
        let mut f = File::create(&gitignore).unwrap();
        writeln!(f, "ignored.txt").unwrap();
        File::create(root.join("ignored.txt")).unwrap();
        File::create(root.join("visible.txt")).unwrap();

        let mut config = LizaConfig::default();
        config.git_ignore = true;

        let flattened = flatten_directory_targets(&[root.to_path_buf()], 1, &config);
        let names: Vec<String> = flattened
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"visible.txt".to_string()));
        assert!(!names.contains(&"ignored.txt".to_string()));
    }
}
