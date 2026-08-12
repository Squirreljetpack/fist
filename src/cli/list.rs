//! `--list` implementations: every `cmd.list` block lives here.
//!
//! The SortOrder arms for listing exist in exactly one place
//! ([`print_sorted`]); output formatting shares [`crate::cli::handlers::print`].
use std::{ffi::OsString, path::PathBuf, process::Command};

use cba::{
    bo::{map_chunks, read_to_chunks},
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
        output
            .output_sep
            .clone()
            .unwrap_or_else(|| "\n".into()),
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
        SortOrder::name => files.sort_by(|a, b| a.file_name().cmp(&b.file_name()).then_with(|| a.cmp(b))),
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
    output: &OutputOpts,
) {
    let (template, sep) = output_parts(output);
    // cwd is abs so results can be added as unchecked
    let files: Vec<PathBuf> = list_dir(cwd, vis, 1).collect();
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
