//! CLI command handlers
use clap::Parser;
use cli_boilerplate_automation::{
    bath::PathExt, bo::map_reader_lines, broc::CommandExt, bs::sort_by_mtime, else_default,
};
use globset::GlobBuilder;
use std::{
    path::PathBuf,
    process::{Command, exit},
};

#[allow(unused_imports)]
use cli_boilerplate_automation::{
    _dbg,
    bait::ResultExt,
    bo::load_type_or_default,
    bog::{BogOkExt, BogUnwrapExt},
    ebog, ibog, prints,
};

use super::{
    matchmaker::mm_get,
    paths::{config_path, current_exe, cwd, home_dir, lessfilter_cfg_path, lz_path, mm_cfg_path},
    tool_types::*,
    types::*,
};
use crate::{
    abspath::AbsPath,
    config::Config,
    db::{
        DbSortOrder, DbTable, Pool, display_entries,
        zoxide::{DbFilter, RetryStrat},
    },
    errors::CliError,
    filters::{SortOrder, Visibility},
    find::{
        fd::{FileTypeArg, build_fd_args},
        walker::list_dir,
    },
    lessfilter,
    run::{
        FsPane,
        globals::{APP, TEMP},
        mm_config::get_mm_cfg,
        start,
    },
    shell::print_shell,
    spawn::{Program, open_wrapped},
    utils::{colors::display_ratatui_colors, filetypes::FileType, path::paths_base},
};
// #[ext(CliResultExt)]
// impl Result<(), CliError> {
//     fn default() -> Self {
//         Err(CliError::Handled)
//     }
// }

pub async fn handle_subcommand(
    cli: Cli,
    cfg: Config,
) -> Result<(), CliError> {
    match cli.subcommand {
        SubCmd::Open(cmd) => handle_open(cli.opts, cmd, cfg).await,
        SubCmd::Apps(cmd) => handle_launch(cli.opts, cmd, cfg).await,
        SubCmd::Files(cmd) => handle_files(cli.opts, cmd, cfg).await,
        SubCmd::Dirs(cmd) => handle_dirs(cli.opts, cmd, cfg).await,
        SubCmd::Fd(cmd) => handle_default(cli.opts, cmd, cfg).await,
        SubCmd::Tools(cmd) => handle_tools(cli.opts, cmd, cfg).await,
        SubCmd::Info(cmd) => handle_info(cli.opts, cmd, cfg).await,
        SubCmd::Rg(cmd) => handle_rg(cli.opts, cmd, cfg).await,
    }
}

async fn handle_open(
    cli: CliOpts,
    cmd: OpenCmd,
    cfg: Config,
) -> Result<(), CliError> {
    // _dbg!(cli, cmd, cfg);
    let pool = Pool::new(cfg.db_path()).await?;
    let conn = pool.get_conn(DbTable::apps).await?;

    open_wrapped(conn, cmd.with.and_then(Program::from_os_string), &cmd.files).await
}

// todo: partitioned info
async fn handle_info(
    cli: CliOpts,
    cmd: InfoCmd,
    cfg: Config,
) -> Result<(), CliError> {
    println!("Config path: {}", config_path().display());
    println!("MM config path: {}", mm_cfg_path().display());
    println!("logs path: {}", cfg.log_path().display());
    println!();

    let is_multi_table_display = cmd.table.is_none();
    let limit = cmd.limit.unwrap_or(50) as u32;

    let pool = Pool::new(cfg.db_path()).await?;
    if let Some(table) = cmd.table {
        let mut conn = pool.get_conn(table).await?;

        let db_filter = DbFilter::new(&cfg.db);
        conn.switch_table(table);
        let entries = conn.get_entries(cmd.sort, &db_filter).await?;

        if cmd.minimal {
            for entry in entries {
                prints!(entry.path.to_string_lossy());
            }
        } else {
            display_entries(&entries);
        }
    }

    Ok(())
}

async fn handle_launch(
    cli: CliOpts,
    cmd: AppsCmd,
    cfg: Config,
) -> Result<(), CliError> {
    // _dbg!(cli, cmd, cfg);
    *APP::TO_OPEN.lock().unwrap() = cmd.files;

    let pane = FsPane::new_launch();

    let mm_cfg_path = cli.mm_config.as_deref().unwrap_or(mm_cfg_path());
    let mm_cfg = get_mm_cfg(mm_cfg_path, &cfg);

    let pool = Pool::new(cfg.global.db_path()).await?;
    start(pane, cfg, mm_cfg, pool).await
    // get entries from db
    // spawn update db
    // lowpri: repopulate on db complete
}

// Need:
async fn handle_files(
    cli: CliOpts,
    cmd: FilesCmd,
    cfg: Config,
) -> Result<(), CliError> {
    // _dbg!(cli, cmd, cfg);
    let input_len = cmd.query.len() as u16;
    let pane = FsPane::Files {
        sort: cmd.sort,
        input: (cmd.query, 0),
    };

    let mm_cfg_path = cli.mm_config.as_deref().unwrap_or(mm_cfg_path());
    let mm_cfg = get_mm_cfg(mm_cfg_path, &cfg);
    let pool = Pool::new(cfg.global.db_path()).await?;
    start(pane, cfg, mm_cfg, pool).await
}

async fn handle_rg(
    cli: CliOpts,
    cmd: RgCommand,
    cfg: Config,
) -> Result<(), CliError> {
    // _dbg!(cli, cmd, cfg);
    todo!()
}
// z behavior:
// with kw: best match
// $1 == . : find
// no args: atime ordered
// keybind cna change the sort order

async fn handle_dirs(
    cli: CliOpts,
    cmd: DirsCmd,
    mut cfg: Config,
) -> Result<(), CliError> {
    let pool = Pool::new(cfg.db_path()).await?;
    if cmd.cd {
        if !cmd.query.is_empty() {
            let conn = pool.get_conn(DbTable::dirs).await?;
            let db_filter = DbFilter::new(&cfg.db).keywords(cmd.query.clone());

            match conn.print_best_by_frecency(&db_filter).await {
                RetryStrat::Next => return Ok(()),
                RetryStrat::None if !cfg.misc.cd_fallback_search => {
                    return Err(CliError::MatchError(matchmaker::MatchError::NoMatch));
                }
                _ => {}
            }
        };
        // fallback to interactive if no match
        // todo: numbers on side to select
        cfg.global.interface.alt_accept = true;
        cfg.global.current.no_multi = true;
        TEMP::set_original_relative_path(cfg.styles.path.relative);
        cfg.styles.path.relative = false;
    } else if let Some(all) = cmd.list {
        let mut conn = pool.get_conn(DbTable::dirs).await?;
        if matches!(all, ListMode::All) {
            cfg.db.filter_missing = true;
        }
        let db_filter = DbFilter::new(&cfg.db).keywords(cmd.query.clone());
        for e in conn.get_entries(cmd.sort, &db_filter).await? {
            match e.path.to_str() {
                Some(s) => {
                    prints!(s)
                }
                None => {
                    if matches!(all, ListMode::All) {
                        prints!(e.path.to_string_lossy())
                    }
                }
            }
        }
        return Ok(());
    }

    let input = if !cmd.query.is_empty() {
        (cmd.query.join(" "), 0)
    } else {
        (String::new(), 0)
    };
    let pane = FsPane::Folders {
        sort: cmd.sort,
        fd_args: vec![],
        input,
    };

    let mm_cfg_path = cli.mm_config.as_deref().unwrap_or(mm_cfg_path());
    let mm_cfg = get_mm_cfg(mm_cfg_path, &cfg);
    start(pane, cfg, mm_cfg, pool).await
}

async fn handle_default(
    cli: CliOpts,
    mut cmd: DefaultCommand,
    mut cfg: Config,
) -> Result<(), CliError> {
    if !cmd.types.is_empty() {
        cmd.vis.dirs = cmd
            .types
            .iter()
            .all(|x| matches!(x, FileTypeArg::Type(FileType::Directory)));
        cmd.vis.files = cmd
            .types
            .iter()
            .all(|x| !matches!(x, FileTypeArg::Type(FileType::Directory)));
    }
    // _dbg!(cli, cmd, cfg);
    let pool = Pool::new(cfg.db_path()).await?;
    let pane = if !atty::is(atty::Stream::Stdin) {
        FsPane::new_stream(AbsPath::new_unchecked(cwd().to_path_buf()), cmd.vis)
    } else
    // if any args are specified
    if cmd.cd {
        cmd.paths.append(&mut cmd.fd); // fd is not supported
        cfg.global.interface.alt_accept = true;
        cfg.global.current.no_multi = true;

        let cwd = AbsPath::new_unchecked(cwd());
        FsPane::new_fd_from_command(cmd, cwd)
    } else
    // any fd arg is specified
    if !cmd.paths.is_empty()
        || !cmd.types.is_empty()
        || cmd.vis != Visibility::default()
        || !cmd.fd.is_empty()
    {
        // pattern specified
        let cwd = if cmd.paths.len() == 1 {
            // last item is a pattern
            AbsPath::new_unchecked(
                if cfg.global.fd.default_search_in_home {
                    home_dir()
                } else {
                    cwd()
                }
                .to_path_buf(),
            )
        } else if cmd.paths.is_empty() {
            if !cmd.fd.iter().any(|x| x == "--glob" || x == "-g") {
                cmd.paths.push(".".into()); // match all pattern
            }
            AbsPath::new_unchecked(cwd().to_path_buf())
        } else {
            AbsPath::new(if cfg.global.fd.reduce_paths {
                paths_base(&cmd.paths[0..cmd.paths.len() - 1])
            } else {
                cmd.paths.remove(0).into()
            })
        };

        let mut conn = pool.get_conn(DbTable::dirs).await?;
        // spawn cost is 1 microsecond
        for path in cmd.paths.iter().take(cmd.paths.len() - 1) {
            conn.bump(&AbsPath::new(path), 1).await._elog();
        }

        if cmd.list {
            let (prog, args) = (
                "fd",
                build_fd_args(
                    cmd.sort.unwrap_or_default(),
                    cmd.vis.validate(),
                    &cmd.types,
                    &cmd.paths,
                    &cmd.fd,
                    &cfg.global.fd,
                ),
            );

            let stdout = else_default!(
                Command::new(prog)
                .args(args)
                .current_dir(&cwd)
                .spawn_piped()
                ._ebog()
                ;
                ?
            );

            let _ = map_reader_lines::<true, CliError>(stdout, move |line| {
                let path = PathBuf::from(line);
                if cmd.vis.filter(&path) {
                    prints!(path.to_string_lossy())
                }
                Ok(())
            });
            return Ok(());
        };

        FsPane::new_fd_from_command(cmd, cwd)
    } else {
        let DefaultCommand { sort, mut vis, .. } = cmd;
        if vis.is_default() {
            vis = cfg.global.panes.nav.default_visibility
        }

        if cmd.list {
            let iter = list_dir(cwd(), cmd.vis, 1); // cwd is abs so we can add results as unchecked
            let sort = sort.unwrap_or_default();

            match sort {
                SortOrder::none => {
                    for (i, path) in iter.enumerate() {
                        prints!(path.to_string_lossy())
                    }
                }
                _ => {
                    let mut files: Vec<PathBuf> = iter.collect();

                    match sort {
                        SortOrder::name => files.sort_by(|a, b| a.file_name().cmp(&b.file_name())),
                        SortOrder::mtime => sort_by_mtime(&mut files),
                        _ => unreachable!(),
                    }

                    for (i, path) in files.into_iter().enumerate() {
                        prints!(path.to_string_lossy())
                    }
                }
            }
            return Ok(());
        };
        FsPane::new_nav(
            AbsPath::new_unchecked(cwd()),
            vis,
            sort.unwrap_or(cfg.global.panes.nav.default_sort),
        )
    };

    let mm_cfg_path = cli.mm_config.as_deref().unwrap_or(mm_cfg_path());
    let mm_cfg = get_mm_cfg(mm_cfg_path, &cfg);
    let pool = Pool::new(cfg.global.db_path()).await?;
    start(pane, cfg, mm_cfg, pool).await
}

async fn handle_tools(
    cli: CliOpts,
    ToolsCmd { tool, args, .. }: ToolsCmd,
    cfg: Config,
) -> Result<(), CliError> {
    // _dbg!(cli, args, cfg);
    let tool = if let Some(x) = tool {
        x
    } else {
        mm_get([
            SubTool::Colors,
            SubTool::Lz { args: args.clone() },
            SubTool::Shell { args: args.clone() },
            SubTool::Lessfilter { args: args.clone() },
            SubTool::Bump { args: args.clone() },
            SubTool::Types { args: args.clone() },
        ])
        .await?
    };

    let executable_err_prefix = "Invalid executable path";

    match tool {
        SubTool::Colors => {
            display_ratatui_colors()?;
            Ok(())
        }
        SubTool::Lz { args } => {
            let mut cmd = Command::new(lz_path());
            cmd.args(args);

            exec(cmd);
        }
        SubTool::Shell { mut args } => {
            // note: this seems to already be the short path of the exe, not that im complaining
            let path = std::env::current_exe().prefix(executable_err_prefix)?;
            let path = path.to_str()._ebog(executable_err_prefix);
            args.insert(0, format!("{path} :tool shell").into());

            let cmd = ShellCommand::parse_from(args);

            print_shell(&cmd, path);
            Ok(())
        }
        SubTool::Lessfilter { mut args } => {
            // sidenote: clap maybe already strips the parent dir so the basename is superfluous
            let path = current_exe().basename();
            args.insert(0, format!("{path} :tool lessfilter").into());

            let cmd = LessfilterCommand::parse_from(args);

            let cfg = load_type_or_default(
                lessfilter_cfg_path(),
                |s| toml::from_str(s),
                include_str!("../../assets/config/lessfilter.toml"),
            );

            lessfilter::handle(cmd, cfg)
        }
        SubTool::Bump { mut args } => {
            let path = current_exe().basename();
            args.insert(0, format!("{path} :tool lessfilter").into());

            let BumpCommand {
                paths,
                count,
                glob: pattern,
                table,
                reset,
            } = BumpCommand::parse_from(args);
            // _dbg!(paths, pattern, count);

            if reset {
                if let Some(table) = table {
                    let mut conn = Pool::new(cfg.global.db_path())
                        .await?
                        .get_conn(table)
                        .await?;
                    conn.reset_table().await?;
                    ibog!("Deleted {table}");
                } else {
                    match std::fs::remove_file(cfg.global.db_path()) {
                        Ok(()) => ibog!("Deleted {}", cfg.global.db_path().to_string_lossy()),
                        Err(e) => ebog!(
                            "Couldn't delete {}: {e}",
                            cfg.global.db_path().to_string_lossy()
                        ),
                    }
                }
                return Ok(());
            }
            // silent errors
            if !paths.is_empty() {
                let mut remove_vec = Vec::with_capacity(paths.len());
                let mut push_vec = Vec::with_capacity(paths.len());

                use globset::{Glob, GlobSetBuilder};
                let mut builder = GlobSetBuilder::new();
                for pattern in &cfg.db.exclude {
                    builder.add(Glob::new(pattern).unwrap());
                }
                let exclude = builder.build().unwrap();

                for path in paths {
                    if !path.exists() {
                        ebog!("{} does not exist!", path.to_string_lossy());
                        exit(1);
                    }
                    let path = AbsPath::new(path);
                    if exclude.is_match(&path) {
                        continue;
                    }
                    if count == 0 {
                        remove_vec.push(path)
                    } else {
                        push_vec.push(path)
                    }
                }

                let mut conn = Pool::new(cfg.global.db_path())
                    .await?
                    .get_conn(DbTable::dirs)
                    .await?;

                if count == 0 {
                    let (dirs, files): (Vec<_>, Vec<_>) =
                        remove_vec.into_iter().partition(|x| x.is_dir());
                    conn.remove_entries(&dirs).await?;
                    if !files.is_empty() {
                        conn.switch_table(DbTable::files);
                        conn.remove_entries(&files).await?;
                    }
                } else {
                    conn.push_files_and_folders(push_vec).await?;
                }
            } else {
                let mut conn = Pool::new(cfg.global.db_path())
                    .await?
                    .get_conn(table.unwrap_or(DbTable::dirs))
                    .await?;

                let glob = GlobBuilder::new(&pattern.unwrap())
                    .build()
                    .__ebog()
                    .compile_matcher();

                let mut to_remove = Vec::new();

                let db_filter = DbFilter::new(&cfg.db);
                let entries = conn
                    .get_entries(DbSortOrder::none, &db_filter)
                    .await
                    .__ebog();
                for e in &entries {
                    if glob.is_match(&e.path) {
                        if count == 0 {
                            to_remove.push(e.path.clone());
                        } else {
                            conn.bump(&e.path, count).await._wlog();
                        }
                    }
                }

                if !to_remove.is_empty() {
                    conn.remove_entries(&to_remove).await?;
                }
            }

            Ok(())
        }
        SubTool::Types { mut args } => {
            let path = current_exe().basename();
            args.insert(0, format!("{path} :tool types").into());

            let TypesCommand { .. } = TypesCommand::parse_from(args);
            todo!()
        }
    }
}

pub fn exec(mut cmd: Command) -> ! {
    let err_prefix = format!("Could not exec {}: ", cmd.get_program().to_string_lossy());
    #[cfg(not(windows))]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        use std::process::exit;

        ebog!("Could not exec lz: {err}");
        exit(1);
    }

    #[cfg(windows)]
    {
        match command.status() {
            Ok(status) => {
                exit(
                    status
                        .code()
                        .unwrap_or(if status.success() { 0 } else { 1 }),
                );
            }
            Err(err) => {
                ebog!("Could not exec lz: {err}");
                exit(1);
            }
        }
    }
}
