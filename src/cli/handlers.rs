//! CLI command handlers
use clap::Parser;
use cli_boilerplate_automation::{
    bait::TransformExt, bath::PathExt, bo::map_reader_lines, broc::CommandExt, bs::sort_by_mtime,
    wbog,
};
use globset::GlobBuilder;
use std::{
    env::{current_dir, set_current_dir},
    path::{MAIN_SEPARATOR_STR, PathBuf},
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
    clap::*,
    clap_tools::*,
    mm_::mm_get,
    paths::{__cwd, __home, config_path, current_exe, lessfilter_cfg_path, liza_path, mm_cfg_path},
};
use crate::{
    abspath::AbsPath,
    cli::SubTool,
    config::Config,
    db::{
        DbSortOrder, DbTable, Pool, display_entries,
        zoxide::{DbFilter, RetryStrat},
    },
    errors::CliError,
    filters::{SortOrder, Visibility},
    find::{
        FileTypeArg,
        fd::{auto_enable_hidden, build_fd_args},
        walker::list_dir,
    },
    lessfilter,
    run::{
        FsPane,
        mm_config::get_mm_cfg,
        start,
        state::{APP, TEMP},
    },
    shell::print_shell,
    spawn::{Program, open_wrapped},
    utils::{
        colors::display_ratatui_colors, filetypes::FileType, path::paths_base, text::path_formatter,
    },
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
    log::debug!("{:?}", cli.subcommand);

    match cli.subcommand {
        SubCmd::Open(cmd) => handle_open(cli.opts, cmd, cfg).await,
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
    mut cfg: Config,
) -> Result<(), CliError> {
    // fs :o or fs :o --with= files
    if cmd.files.is_empty() || cmd.with.as_ref().is_some_and(|s| s.is_empty()) {
        *APP::TO_OPEN.lock().unwrap() = cmd.files;
        cfg.global.interface.no_multi_accept = true;
        let pane = FsPane::new_launch();

        let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);

        let pool = Pool::new(cfg.db_path()).await?;
        start(pane, cfg, mm_cfg, pool).await
    } else {
        let pool = Pool::new(cfg.db_path()).await?;
        let conn = pool.get_conn(DbTable::apps).await?;

        let prog = cmd.with.and_then(Program::from_os_string);
        if prog.is_none() {
            crate::spawn::init_spawn_with(cfg.misc.spawn_with);
        }

        open_wrapped(conn, prog, &cmd.files, false).await
    }
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

    let limit = cmd.limit.unwrap_or(if cmd.minimal { 0 } else { 50 });

    let pool = Pool::new(cfg.db_path()).await?;
    if let Some(table) = cmd.table {
        let mut conn = pool.get_conn(table).await?;

        conn.switch_table(table);
        let mut entries = conn.get_entries_range(0, 0, cmd.sort).await?;
        if matches!(cmd.sort, DbSortOrder::frecency) {
            let now = chrono::Utc::now().timestamp();
            entries.sort_by_key(|e| std::cmp::Reverse(DbFilter::_score(now, e)));
        }
        if limit != 0 {
            entries.truncate(limit);
        }

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

// Need:
async fn handle_files(
    cli: CliOpts,
    cmd: FilesCmd,
    cfg: Config,
) -> Result<(), CliError> {
    // _dbg!(cli, cmd, cfg);
    let pane = FsPane::Files {
        sort: cmd.sort,
        input: (cmd.query, 0),
    };

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    let pool = Pool::new(cfg.db_path()).await?;
    start(pane, cfg, mm_cfg, pool).await
}

async fn handle_rg(
    cli: CliOpts,
    mut cmd: RgCommand,
    cfg: Config,
) -> Result<(), CliError> {
    if cmd.vis.is_default() {
        cmd.vis = cfg.global.panes.rg.default_visibility
    }

    todo!()
}

async fn handle_dirs(
    cli: CliOpts,
    mut cmd: DirsCmd,
    mut cfg: Config,
) -> Result<(), CliError> {
    let pool = Pool::new(cfg.db_path()).await?;
    if cmd.cd && cmd.list.is_some() {
        return Err(CliError::ConflictingFlags("cd", "list"));
    }

    if cmd.cd {
        cfg.history.show_missing = false;

        if !cmd.query.is_empty() {
            let conn = pool.get_conn(DbTable::dirs).await?;
            let db_filter = DbFilter::new(&cfg.history).with_keywords(cmd.query.clone());

            match conn.print_best_by_frecency(&db_filter).await {
                RetryStrat::Next => return Ok(()),
                RetryStrat::None if !cfg.misc.cd_fallback_search => {
                    return Err(CliError::MatchError(matchmaker::MatchError::NoMatch));
                }
                _ => {
                    cmd.query.truncate(1); // since no match, truncating is more desirable
                }
            }
        };

        // fallback to interactive if no match
        cfg.global.interface.alt_accept = true;
        cfg.global.interface.no_multi_accept = true;
        TEMP::set_initial_relative_path(cfg.styles.path.relative);
        cfg.styles.path.relative = false;
    } else if let Some(all) = cmd.list {
        let mut conn = pool.get_conn(DbTable::dirs).await?;

        if matches!(all, ListMode::All) {
            cfg.history.show_missing = true;
        }
        let db_filter = DbFilter::new(&cfg.history).with_keywords(cmd.query.clone());

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
        input,
    };

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    start(pane, cfg, mm_cfg, pool).await
}

async fn handle_default(
    cli: CliOpts,
    mut cmd: DefaultCommand,
    mut cfg: Config,
) -> Result<(), CliError> {
    // check input
    let cli_set_ignore = cmd.vis.ignore;
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
    if cmd.cd && cmd.list {
        return Err(CliError::ConflictingFlags("cd", "list"));
    }

    let pool = Pool::new(cfg.db_path()).await?;

    let pane = if
    // piped input
    !atty::is(atty::Stream::Stdin) && !cmd.no_read && !cmd.list {
        if cmd.cd {
            cfg.global.interface.alt_accept = true;
            cfg.history.show_missing = false;

            // stream can only occur as the first pane, this ensures paths are not modified in display
            TEMP::set_initial_relative_path(cfg.styles.path.relative);
            cfg.styles.path.relative = false;
        };
        FsPane::new_stream(AbsPath::new_unchecked(__cwd()), cmd.vis)
    } else if cmd.cd {
        if !cmd.fd.is_empty() && !cmd.paths.is_empty() {
            wbog!(
                "fd_args are not supported with --cd: to avoid confusion, specify all paths after --."
            )
        }
        cmd.paths.append(&mut cmd.fd); // fd opts are not supported
        cfg.global.interface.alt_accept = true;
        cfg.history.show_missing = false;

        let nav_pane = cmd.paths.last().is_some_and(|s| {
            s.to_str().is_some_and(|s| {
                s.strip_prefix('.')
                    .is_some_and(|s| s == MAIN_SEPARATOR_STR || s == "/")
            })
        });

        if cmd.vis.is_default() {
            cmd.vis = if nav_pane {
                cfg.global.panes.nav.default_visibility
            } else {
                cfg.global.panes.fd.default_visibility
            }
        }

        // determine cwd
        let cwd = if cmd.paths.len() > 1
        // treat paths as zoxide args (since searching over multiple paths should be uncommon with --cd)
        {
            let conn = pool.get_conn(DbTable::dirs).await?;

            // the last path is the pattern, so determine the best match from keywords formed by all but the last
            let num_keywords = cmd.paths.len() - 1;

            let kw: Vec<String> = cmd
                .paths
                .drain(..num_keywords)
                .map(|f| f.to_string_lossy().into_owned())
                .collect();

            let db_filter = DbFilter::new(&cfg.history).with_keywords(kw.clone());

            match conn.return_best_by_frecency(&db_filter).await {
                None => {
                    if cfg.misc.cd_fallback_search && !cmd.list {
                        // todo: lowpri: relaunch this binary with :dir and get its result?
                        // let input = (kw.last().cloned().unwrap_or_default(), 0);
                        // let pane = FsPane::Folders {
                        //     sort: DbSortOrder::frecency,
                        //     input,
                        // };
                        // let mm_cfg_path = cli.mm_config.as_deref().unwrap_or(mm_cfg_path());
                        // let mm_cfg = get_mm_cfg(mm_cfg_path, &cfg);
                        // start(pane, cfg, mm_cfg, pool).await
                        //
                        return Err(CliError::MatchError(matchmaker::MatchError::NoMatch));
                    } else {
                        return Err(CliError::MatchError(matchmaker::MatchError::NoMatch));
                    }
                }
                Some(p) => {
                    set_current_dir(&p)
                        .prefix("Failed to change directory")
                        ._elog();
                    p
                }
            }
        } else
        // if only pattern is given, determine a directory as follows:
        {
            if cmd.paths.is_empty() {
                cmd.paths.push("..".into())
            } else if !nav_pane && cfg.global.fd.default_search_ignore && !cli_set_ignore {
                cmd.vis.ignore = true;
            }

            // the z shell function passes through here when the last provided argument is ., .. or ./, corresponding to:
            // - `.`: search all directories in default_dir
            // - `..` (only argument): same as 1, but default_dir is forced to cwd
            // - `./`: show all directories in current dir
            // ..which is analgous to the behavior !cmd.cd, except that the analgue of 3 is the no-arg branch rather than `./`
            // Note: another parsing approach is tor replace initial .. to . but that seems more confusing.
            let force_search_in_cwd = nav_pane || cmd.paths[0].cmp_exch("..", ".".into());

            AbsPath::new_unchecked(
                if !force_search_in_cwd && cfg.global.fd.default_search_in_home {
                    __home()
                } else {
                    __cwd()
                },
            )
        };

        if nav_pane {
            FsPane::new_nav(
                cwd,
                cmd.vis,
                cmd.sort.unwrap_or(cfg.global.panes.nav.default_sort),
            )
        } else
        // interactively search the best match
        {
            FsPane::new_fd_from_command(cmd, cwd)
        }
    } else if
    // any fd arg is specified
    !cmd.paths.is_empty()
        || !cmd.types.is_empty()
        || cmd.vis != Visibility::default()
        || !cmd.fd.is_empty()
    {
        if cmd.vis.is_default() {
            cmd.vis = cfg.global.panes.fd.default_visibility
        }

        // pattern specified
        let cwd = if cmd.paths.len() == 1 {
            if cfg.global.fd.default_search_ignore && !cli_set_ignore {
                cmd.vis.ignore = true;
            }

            // support `..` as a shorthand for 'search (any pattern in) current directory'
            let force_search_in_cwd = cmd.paths[0].cmp_exch("..", ".".into());

            // last item is a pattern
            AbsPath::new_unchecked(
                if !force_search_in_cwd && cfg.global.fd.default_search_in_home {
                    __home()
                } else {
                    __cwd()
                }
                .to_path_buf(),
            )
        } else if cmd.paths.is_empty() {
            if !cmd.fd.iter().any(|x| x == "--glob" || x == "-g") {
                cmd.paths.push(".".into()); // match all pattern
            }
            AbsPath::new_unchecked(__cwd().to_path_buf())
        } else {
            AbsPath::new_unchecked(if cfg.global.fd.reduce_paths {
                paths_base(&cmd.paths[0..cmd.paths.len() - 1])
            } else {
                cmd.paths.remove(0).abs(current_dir().__ebog())
            })
        };

        // set the cwd determined above
        set_current_dir(&cwd)
            .prefix(format!("Failed to enter {}", cwd.to_string_lossy()))
            .__ebog();

        // bump paths in db
        let paths = cmd.paths[..cmd.paths.len().saturating_sub(1)].to_vec();
        tokio::spawn(async move {
            if let Ok(mut conn) = pool.get_conn(DbTable::dirs).await {
                for path in paths {
                    conn.bump(AbsPath::new(path), 1).await._elog();
                }
            }
        });

        if cmd.list {
            // mirror new_fd behavior
            if auto_enable_hidden(&cmd.paths) {
                cmd.vis.hidden = true;
            }

            let (prog, args) = (
                "fd",
                build_fd_args(
                    cmd.vis.validated(),
                    &cmd.types,
                    &cmd.paths,
                    &cmd.fd,
                    &cfg.global.fd,
                ),
            );

            let stdout = match Command::new(prog).args(args).spawn_piped()._ebog() {
                Some(s) => s,
                None => return Err(CliError::Handled),
            };

            let _ = map_reader_lines::<true, CliError>(stdout, move |line| {
                let path = PathBuf::from(line);
                let mut push = true;
                // most checks were already handled by fd
                if cmd.vis.hidden_files && path.is_hidden() {
                    if cmd.vis.dirs {
                        push = path.is_dir();
                    } else {
                        push = !path.is_dir()
                    }
                };
                if !cmd.vis.all() {
                    push = path.exists()
                }
                if push {
                    if let Some(template) = &cmd.list_fmt {
                        let s = path_formatter(template, &AbsPath::new(path));
                        prints!(s)
                    } else {
                        prints!(path.to_string_lossy())
                    }
                }
                Ok(())
            });
            return Ok(());
        };

        FsPane::new_fd_from_command(cmd, cwd)
    } else {
        let DefaultCommand { sort, mut vis, .. } = cmd;
        if vis.is_default() {
            vis = cfg.global.panes.nav.default_visibility;
        }

        if cmd.list {
            let iter = list_dir(__cwd(), cmd.vis, 1); // cwd is abs so we can add results as unchecked
            let sort = sort.unwrap_or_default();

            match sort {
                SortOrder::none => {
                    for path in iter {
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

                    for path in files.into_iter() {
                        prints!(path.to_string_lossy())
                    }
                }
            }
            return Ok(());
        };
        FsPane::new_nav(
            AbsPath::new_unchecked(__cwd()),
            vis,
            sort.unwrap_or(cfg.global.panes.nav.default_sort),
        )
    };

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    let pool = Pool::new(cfg.db_path()).await?;
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
            SubTool::Liza { args: args.clone() },
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
        SubTool::ShowBinds => {
            let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
            let help_str = matchmaker::binds::display_binds(&mm_cfg.binds, None);
            prints!(help_str.to_string());
            Ok(())
        }
        SubTool::Liza { args } => Command::new(liza_path()).args(args)._exec(),
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

            let cfg = load_type_or_default(lessfilter_cfg_path(), |s| toml::from_str(s));

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
                    let mut conn = Pool::new(cfg.db_path()).await?.get_conn(table).await?;
                    conn.reset_table().await?;
                    ibog!("Deleted {table}");
                } else {
                    match std::fs::remove_file(cfg.db_path()) {
                        Ok(()) => ibog!("Deleted {}", cfg.db_path().to_string_lossy()),
                        Err(e) => ebog!("Couldn't delete {}: {e}", cfg.db_path().to_string_lossy()),
                    }
                }
                return Ok(());
            }
            // silent errors
            if !paths.is_empty() {
                let mut entry_queue = Vec::with_capacity(paths.len());

                // don't bump exclusions, but do remove them
                let exclude = if count != 0 {
                    use globset::{Glob, GlobSetBuilder};
                    let mut builder = GlobSetBuilder::new();
                    for pattern in &cfg.history.exclude {
                        builder.add(Glob::new(pattern).prefix("Error in cfg.history.exclude")?);
                    }
                    Some(builder.build().prefix("Error in cfg.history.exclude")?)
                } else {
                    None
                };

                for path in paths {
                    if !path.exists() {
                        ebog!("{} does not exist!", path.to_string_lossy());
                        exit(1);
                    }
                    let path = AbsPath::new_canonical(path);
                    if exclude.as_ref().is_some_and(|e| e.is_match(&path)) {
                        continue;
                    }
                    entry_queue.push(path)
                }

                let mut conn = Pool::new(cfg.db_path())
                    .await?
                    .get_conn(DbTable::dirs)
                    .await?;

                if count == 0 {
                    let (dirs, files): (Vec<_>, Vec<_>) =
                        entry_queue.into_iter().partition(|x| x.is_dir());

                    // let dirs_removed = conn.remove_entries(&dirs).await?; // can't use this as it doesn't resolve symlinks
                    let dirs_removed = conn.remove_paths(&dirs).await?;
                    let files_removed = conn.remove_paths(&files).await?;

                    let mut msg = String::new();
                    if !files.is_empty() {
                        msg.push_str(&format!("{} files", files_removed));
                    }
                    if !dirs.is_empty() {
                        if !msg.is_empty() {
                            msg.push_str(" and ");
                        }
                        msg.push_str(&format!("{} directories", dirs_removed));
                    }

                    if !msg.is_empty() {
                        ibog!("Removed {}.", msg);
                    }
                } else {
                    conn.push_files_and_folders(entry_queue).await?;
                }
            } else {
                // glob is per-table
                let mut conn = Pool::new(cfg.db_path())
                    .await?
                    .get_conn(table.unwrap_or(DbTable::dirs))
                    .await?;

                let glob = GlobBuilder::new(&pattern.unwrap())
                    .build()
                    .__ebog()
                    .compile_matcher();

                let mut to_remove = Vec::new();

                let db_filter = DbFilter::new(&cfg.history);
                let entries = conn
                    .get_entries(DbSortOrder::none, &db_filter)
                    .await
                    .__ebog();

                let mut matched = 0;
                for e in entries {
                    if glob.is_match(&e.path) {
                        matched += 1;
                        if count == 0 {
                            to_remove.push(e.path.clone());
                        } else {
                            conn.bump(e.path, count).await._wlog();
                        }
                    }
                }

                log::debug!("Matched {matched} paths.");
                if count == 0 {
                    let removed_count = conn.remove_entries(&to_remove).await?;
                    ibog!("Removed {removed_count} entries.");
                } else {
                    ibog!("Matched {matched} paths.");
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
