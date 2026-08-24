//! `--list` implementations: every `cmd.list` block lives here.
//!
//! The SortOrder arms for listing exist in exactly one place
//! ([`print_sorted`]); output formatting shares [`crate::cli::handlers::print`].
use std::{ffi::OsString, path::PathBuf, process::Command};

use ansi_to_tui::IntoText;
use cba::{
    bo::{map_chunks, map_reader_lines, read_to_chunks},
    bog::BogOkExt,
    broc::CommandExt,
    prints,
};
use fist_types::{
    When,
    filetypes::FileTypeArg,
    filters::{SortOrder, Visibility},
};

use super::{OutputOpts, clap_helpers::ListMode, handlers::print};
use crate::{
    cli::paths::__cwd,
    config::Config,
    db::{Connection, zoxide::HistoryConfig},
    errors::CliError,
    find::{fd::build_fd_args, metadata, rg::build_rg_args, walker::list_dir},
};

/// Resolve `cli.output` into (template, separator).
fn output_parts(output: &OutputOpts) -> (Option<String>, String) {
    (
        output.format.clone(),
        output.output_sep.clone().unwrap_or_else(|| "\n".into()),
    )
}

/// Sort + print a collected `Vec` — the only place the SortOrder arms for
/// listing exist. `none` preserves populate order.
fn print_sorted(
    mut files: Vec<PathBuf>,
    sort: SortOrder,
    template: &Option<String>,
    sep: &str,
) {
    match sort {
        SortOrder::none => {}
        SortOrder::name => {
            files.sort_by(|a, b| a.file_name().cmp(&b.file_name()).then_with(|| a.cmp(b)))
        }
        SortOrder::mtime => metadata::sort_by_mtime(&mut files),
        SortOrder::atime => metadata::sort_by_atime(&mut files),
        SortOrder::size => metadata::sort_by_size(&mut files),
    }
    for path in &files {
        print(path, template, sep);
    }
}

/// Nav pane listing (`fs --list` with no fd args): `list_dir` + sort + print.
pub fn nav_list(
    cwd: &std::path::Path,
    vis: Visibility,
    sort: SortOrder,
    ignore_patterns: &[String],
    output: &OutputOpts,
) {
    let (template, sep) = output_parts(output);
    // cwd is abs so results can be added as unchecked
    let files: Vec<PathBuf> = list_dir(cwd, vis, 1, ignore_patterns).collect();
    print_sorted(files, sort, &template, &sep);
}

/// fd-backed listing (`fs [pattern] --list`): spawn fd, stream-filter, print.
pub fn fd_list(
    vis: Visibility,
    types: &[FileTypeArg],
    paths: &[OsString],
    fd_args: &[OsString],
    cfg: &Config,
    output: &OutputOpts,
) -> Result<(), CliError> {
    let (prog, args) = (
        "fd",
        build_fd_args(vis, types, paths, fd_args, &cfg.global.fd),
    );

    let (_child, stdout) = match Command::new(prog).args(args).spawn_piped()._ebog() {
        Some(s) => s,
        None => return Err(CliError::Handled),
    };

    let (template, output_sep) = output_parts(output);
    let list_absolute_paths = cfg.misc.list_absolute_paths;

    let _ = map_chunks::<CliError>(
        read_to_chunks(stdout, '\0'),
        move |line| {
            let path = if list_absolute_paths {
                __cwd().join(PathBuf::from(line))
            } else {
                PathBuf::from(line)
            };
            let push = vis.post_fd_filter(&path);

            if push {
                print(&path, &template, &output_sep)
            }
            Ok(())
        },
        true,
    );
    Ok(())
}

/// rg-backed listing (`fs :rg --list`): spawn rg, stream-filter, print.
/// Sorting is delegated to rg's own `--sort`/`--sortr` flags.
#[allow(clippy::too_many_arguments)]
pub fn rg_list(
    vis: Visibility,
    sort: SortOrder,
    context: [usize; 2],
    case: When,
    no_heading: bool,
    fixed_strings: bool,
    patterns: &[String],
    paths: &[PathBuf],
    rg: &[OsString],
    cfg: &Config,
    output: &OutputOpts,
) -> Result<(), CliError> {
    let (prog, args) = (
        "rg",
        build_rg_args(
            vis,
            sort,
            context,
            case,
            no_heading,
            fixed_strings,
            patterns,
            paths,
            rg,
            &cfg.global.rg,
        ),
    );

    let (_child, stdout) = match Command::new(prog).args(args).spawn_piped()._ebog() {
        Some(s) => s,
        None => return Err(CliError::Handled),
    };

    let (template, output_sep) = output_parts(output);
    let list_absolute_paths = cfg.misc.list_absolute_paths;

    if no_heading {
        let mut path_buffer = String::new();
        let _ = map_reader_lines::<CliError>(
            stdout,
            move |line| {
                if let Some((p, _)) = line.split_once('\0') {
                    let raw_path = if path_buffer.is_empty() {
                        p.to_string()
                    } else {
                        let mut s = std::mem::take(&mut path_buffer);
                        s.push_str(p);
                        s
                    };
                    let path_str = raw_path
                        .as_bytes()
                        .into_text()
                        .map(|x| crate::utils::text::text_to_string(&x))
                        .unwrap_or(raw_path);
                    let path = if list_absolute_paths {
                        __cwd().join(PathBuf::from(&path_str))
                    } else {
                        PathBuf::from(&path_str)
                    };

                    let push = vis.post_fd_filter(&path);

                    if push {
                        print(&path, &template, &output_sep)
                    }
                } else {
                    path_buffer.push_str(&line);
                    path_buffer.push('\n');
                }
                Ok(())
            },
            true,
        );
    } else {
        let mut current_path = String::new();
        let mut path_buffer = String::new();
        let _ = map_reader_lines::<CliError>(
            stdout,
            move |line| {
                if current_path.is_empty() {
                    if let Some((p, _)) = line.split_once('\0') {
                        let raw_path = if path_buffer.is_empty() {
                            p.to_string()
                        } else {
                            let mut s = std::mem::take(&mut path_buffer);
                            s.push_str(p);
                            s
                        };
                        let path_str = raw_path
                            .as_bytes()
                            .into_text()
                            .map(|x| crate::utils::text::text_to_string(&x))
                            .unwrap_or(raw_path);
                        if !path_str.is_empty() {
                            let path = if list_absolute_paths {
                                __cwd().join(PathBuf::from(&path_str))
                            } else {
                                PathBuf::from(&path_str)
                            };

                            let push = vis.post_fd_filter(&path);
                            if push {
                                print(&path, &template, &output_sep);
                            }
                            current_path = path_str;
                        }
                    } else {
                        path_buffer.push_str(&line);
                        path_buffer.push('\n');
                    }
                } else if line.is_empty() {
                    current_path.clear();
                    path_buffer.clear();
                }
                Ok(())
            },
            true,
        );
    }

    Ok(())
}

/// db-backed directory listing (`fs :d --list`): rows are already ordered by
/// the query (`sort`), so this only formats and prints them.
///
/// Non-UTF8 paths are skipped unless `mode` is [`ListMode::All`].
pub async fn dirs_list(
    conn: &mut Connection,
    sort: SortOrder,
    db_filter: &HistoryConfig,
    mode: ListMode,
    output: &OutputOpts,
) -> Result<(), CliError> {
    let (template, sep) = output_parts(output);
    let all = matches!(mode, ListMode::All);

    for e in conn
        .get_entries(sort, db_filter, crate::db::DbTable::dirs)
        .await?
    {
        match e.path.to_str() {
            Some(_) => print(&e.path, &template, &sep),
            None => {
                if all {
                    prints!(e.path.to_string_lossy())
                }
            }
        }
    }
    Ok(())
}
