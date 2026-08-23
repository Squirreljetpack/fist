pub mod entry;
pub mod tree;
pub mod walker;

use std::{
    io::{self, Cursor, IsTerminal, Write},
    path::PathBuf,
};

use super::super::config::{LizaConfig, ViewMode};
use crate::{errors::CliError, pager::page_reader};
use tree::render_tree;
use walker::{build_file_entry, build_tree_node, collect_dir_entries};

pub fn run(config: &LizaConfig) -> Result<(), CliError> {
    if config.view_mode == Some(ViewMode::Git) {
        return super::eza::run(config);
    }

    let target_paths = if config.paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        config.paths.clone()
    };

    let is_tree_mode = config.unbounded_tree
        || config.tree_depth.is_some()
        || matches!(
            config.view_mode,
            Some(ViewMode::Nav) | Some(ViewMode::Tree) | Some(ViewMode::Dirs)
        );

    let is_paged_view = matches!(
        config.view_mode,
        Some(ViewMode::Nav) | Some(ViewMode::Tree) | Some(ViewMode::Dirs)
    );

    let mut buf = Vec::new();
    if is_tree_mode {
        render_tree_output(&target_paths, config, &mut buf)?;
    } else if config.view_mode == Some(ViewMode::Flatten) {
        render_flatten_output(&target_paths, config, &mut buf)?;
    } else {
        render_flat_output(&target_paths, config, &mut buf)?;
    }

    if is_paged_view && io::stdout().is_terminal() {
        page_reader(Cursor::new(buf), false, None).map_err(CliError::IoError)?;
    } else {
        let mut stdout = io::stdout().lock();
        stdout.write_all(&buf)?;
    }

    Ok(())
}

fn format_header(config: &LizaConfig) -> Option<String> {
    if !config.header || config.no_header {
        return None;
    }

    let mut headers = Vec::new();
    if config.show_octal
        || !config.no_permissions
        || config.show_clean_long
        || config.show_extensive
    {
        headers.push("Permissions");
    }
    if !config.no_filesize
        || config.show_size
        || config.show_clean_long
        || config.show_extensive
        || config.view_mode == Some(ViewMode::Dirs)
    {
        headers.push("Size");
    }
    if config.show_time {
        headers.push("Date Created");
        headers.push("Date Modified");
        headers.push("Date Accessed");
    } else if !config.no_time
        || config.show_mtime
        || config.show_clean_long
        || config.show_extensive
    {
        headers.push("Date Modified");
    }
    if config.show_extensive {
        headers.push("Type");
    }

    if headers.is_empty() {
        Some("Name".to_string())
    } else {
        Some(format!("({}) Name", headers.join(" | ")))
    }
}

fn render_tree_output(
    paths: &[PathBuf],
    config: &LizaConfig,
    writer: &mut impl Write,
) -> Result<(), CliError> {
    if let Some(header) = format_header(config) {
        writeln!(writer, "{header}")?;
    }

    let max_depth = if config.unbounded_tree {
        usize::MAX
    } else if let Some(depth) = config.tree_depth {
        depth
    } else if config.view_mode == Some(ViewMode::Nav) || config.view_mode == Some(ViewMode::Dirs) {
        std::env::var("MAX_DEPTH")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3)
    } else {
        3
    };

    let mut roots = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let node = build_tree_node(path, name, 0, max_depth, config);
        roots.push(node);
    }

    render_tree(&roots, writer)?;
    Ok(())
}

fn render_flatten_output(
    paths: &[PathBuf],
    config: &LizaConfig,
    writer: &mut impl Write,
) -> Result<(), CliError> {
    if let Some(header) = format_header(config) {
        writeln!(writer, "{header}")?;
    }

    let max_depth = if config.unbounded_tree {
        usize::MAX
    } else {
        config.tree_depth.unwrap_or(1)
    };

    let flattened = super::eza::flatten_directory_targets(paths, max_depth, config);
    for path in flattened {
        let name = path.to_string_lossy().to_string();
        let is_dir = path.is_dir();
        let file_entry = build_file_entry(name, &path, is_dir, config);
        writeln!(writer, "{file_entry}")?;
    }

    Ok(())
}

fn render_flat_output(
    paths: &[PathBuf],
    config: &LizaConfig,
    writer: &mut impl Write,
) -> Result<(), CliError> {
    let multiple_targets = paths.len() > 1;
    let header = format_header(config);

    if !multiple_targets {
        if let Some(ref hdr) = header {
            writeln!(writer, "{hdr}")?;
        }
    }

    for (i, path) in paths.iter().enumerate() {
        if !path.exists() {
            writeln!(writer, "{}: No such file or directory", path.display())?;
            continue;
        }

        if path.is_dir() {
            if multiple_targets {
                if i > 0 {
                    writeln!(writer)?;
                }
                writeln!(writer, "{}:", path.display())?;
                if let Some(ref hdr) = header {
                    writeln!(writer, "{hdr}")?;
                }
            }

            let entries = collect_dir_entries(path, config);
            for entry in entries {
                let name = entry.file_name().to_string_lossy().to_string();
                let file_entry =
                    build_file_entry(name, &entry.path(), entry.path().is_dir(), config);
                writeln!(writer, "{file_entry}")?;
            }
        } else {
            let name = path.to_string_lossy().to_string();
            let file_entry = build_file_entry(name, path, false, config);
            writeln!(writer, "{file_entry}")?;
        }
    }

    Ok(())
}
