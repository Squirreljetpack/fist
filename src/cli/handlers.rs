//! CLI command handlers
use clap::Parser;
use cli_boilerplate_automation::{
    bait::{MaybeExt, TransformExt},
    bath::PathExt,
    bo::map_reader_lines,
    broc::CommandExt,
    bs::sort_by_mtime,
    ibog, wbog,
};
use globset::GlobBuilder;
use std::{
    env::{current_dir, set_current_dir},
    path::{MAIN_SEPARATOR_STR, PathBuf},
    process::{Command, exit},
};

#[allow(unused_imports)]
use cli_boilerplate_automation::{
    _dbg, _ibog,
    bait::ResultExt,
    bo::load_type_or_default,
    bog::{BogOkExt, BogUnwrapExt},
    ebog, prints,
};

use super::{
    clap::*,
    clap_tools::*,
    mm_::mm_get,
    paths::{__cwd, __home, config_path, current_exe, lessfilter_cfg_path, liza_path, mm_cfg_path},
};
use crate::{
    abspath::AbsPath,
    cli::{SubTool, clap_helpers::ListMode, env::EnvOpts},
    config::Config,
    db::{
        DbSortOrder, DbTable, Pool, display_entries,
        zoxide::{DbFilter, RetryStrat},
    },
    errors::{CliError, DbError},
    find::{
        fd::{auto_enable_hidden, build_fd_args},
        rg::build_rg_args,
        walker::list_dir,
    },
    lessfilter::{self, LessfilterConfig},
    run::{
        FsPane,
        mm_config::get_mm_cfg,
        start,
        stash::{CustomStashActionActionState, STASH},
        state::{InitialRelativePathSetting, TlsStore},
    },
    shell::print_shell,
    spawn::{Program, open_wrapped},
    utils::{colors::display_ratatui_styles, formatter::format_path, path::paths_base},
};
use fist_types::filetypes::{FileType, FileTypeArg};
use fist_types::filters::{SortOrder, Visibility};

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
    let pool = Pool::new(cfg.db_path()).await?;

    // fs :o or fs :o --with= files
    if cmd.files.is_empty() || cmd.with.as_ref().is_some_and(|s| s.is_empty()) {
        STASH::set_cas(CustomStashActionActionState::App);
        for path in cmd.files {
            STASH::push_custom(AbsPath::new_unchecked(path));
        }
        TlsStore::set(CustomStashActionActionState::default());

        cfg.global.interface.no_multi_accept = true;
        let pane = FsPane::new_launch();

        let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);

        start(pane, cfg, mm_cfg, pool, cli.enter_prompt).await
    } else {
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
    _cli: CliOpts,
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

        let template = EnvOpts::with_env(|s| s.output_template.clone());
        let output_separator =
            EnvOpts::with_env(|s| s.output_separator.clone()).unwrap_or("\n".into());
        if cmd.minimal {
            for entry in entries {
                print(&entry.path, &template, &output_separator);
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
    let pane = FsPane::Files {
        sort: cmd.sort,
        input: (cmd.query, 0),
    };

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    let pool = Pool::new(cfg.db_path()).await?;
    start(pane, cfg, mm_cfg, pool, cli.enter_prompt).await
}

async fn handle_rg(
    cli: CliOpts,
    mut cmd: RgCommand,
    mut cfg: Config,
) -> Result<(), CliError> {
    let vis = Visibility::from_cmd_or_cfg(cmd.vis, cfg.global.panes.search.default_visibility);

    let sort = cmd.sort.unwrap_or(
        cfg.global
            .panes
            .search
            .default_sort
            .unwrap_or(SortOrder::none),
    );

    if cmd._no_heading_alias {
        cmd.no_heading = Some(true);
    };
    cfg.global.panes.search.no_heading._take(cmd.no_heading);
    let no_heading = cfg.global.panes.search.no_heading;

    if cmd.list {
        let (prog, args) = (
            "rg",
            build_rg_args(
                vis,
                sort,
                cmd.context.resolve(),
                cmd.case.resolve(),
                no_heading,
                cmd.fixed_strings,
                &cmd.patterns,
                &cmd.paths,
                &cmd.rg,
                &cfg.global.rg,
            ),
        );

        let stdout = match Command::new(prog).args(args).spawn_piped()._ebog() {
            Some(s) => s,
            None => return Err(CliError::Handled),
        };

        let template = EnvOpts::with_env(|s| s.output_template.clone());
        let output_separator =
            EnvOpts::with_env(|s| s.output_separator.clone()).unwrap_or("\n".into());

        let _ = map_reader_lines::<true, CliError>(stdout, move |line| {
            let path = PathBuf::from(line);
            let push = vis.post_fd_filter(&path);

            if push {
                print(&path, &template, &output_separator)
            }
            Ok(())
        });
        return Ok(());
    };

    let pool = Pool::new(cfg.db_path()).await?;

    let pane = FsPane::new_rg_full(
        AbsPath::default(),
        sort,
        vis,
        cmd.paths,
        cmd.query,
        cmd.patterns,
        cmd.filtering,
        cmd.context.resolve(),
        cmd.case.resolve(),
        no_heading,
        cmd.fixed_strings,
        cmd.rg,
    );

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    start(pane, cfg, mm_cfg, pool, cli.enter_prompt).await
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

            let result = conn.print_best_by_frecency(&db_filter).await;
            match result {
                RetryStrat::Next => return Ok(()),
                RetryStrat::None if db_filter.refind != RetryStrat::Search => {
                    return Err(CliError::MatchError(matchmaker::MatchError::NoMatch));
                }
                _ => {
                    if matches!(result, RetryStrat::None) {
                        // since no match, truncating is more desirable
                        cmd.query.truncate(1);
                    }
                }
            }
        };

        // fallback to interactive if no match
        cfg.global.interface.alt_accept = true;
        cfg.global.interface.no_multi_accept = true;
        TlsStore::set(InitialRelativePathSetting(cfg.styles.path.relative));
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

    let input = if !cmd.initial_input.is_empty() {
        (cmd.initial_input, 0)
    } else if !cmd.query.is_empty() {
        (cmd.query.join(" "), 0)
    } else {
        (String::new(), 0)
    };
    let pane = FsPane::Folders {
        sort: cmd.sort,
        input,
    };

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    start(pane, cfg, mm_cfg, pool, cli.enter_prompt).await
}

async fn handle_default(
    cli: CliOpts,
    mut cmd: DefaultCommand,
    mut cfg: Config,
) -> Result<(), CliError> {
    // check input
    let mut is_default_dir = true;
    if !cmd.types.is_empty() {
        if cmd
            .types
            .iter()
            .all(|x| matches!(x, FileTypeArg::Type(FileType::Directory)))
        {
            if cmd.vis.files == Some(false) {
                wbog!("Overriding vis.dirs to true due to -t d")
            }
            cmd.vis.dirs = Some(true)
        }
        if cmd
            .types
            .iter()
            .all(|x| !matches!(x, FileTypeArg::Type(FileType::Directory)))
        {
            if cmd.vis.files == Some(false) {
                wbog!("Overriding vis.files to true due to -t")
            }
            cmd.vis.files = Some(true)
        }
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
            TlsStore::set(cfg.styles.path.relative);
            cfg.styles.path.relative = false;
        };
        FsPane::new_stream(AbsPath::new_unchecked(__cwd()), cmd.vis.into(), true)
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
                    if !matches!(db_filter.refind, RetryStrat::Search) && !cmd.list {
                        return Err(CliError::MatchError(matchmaker::MatchError::NoMatch));
                    } else {
                        ibog!("Searching from `fs :dir` due to `refind = Search`");
                        let sort = cmd.sort.unwrap_or(if nav_pane {
                            cfg.global.panes.nav.default_sort
                        } else {
                            Default::default()
                        });

                        let cmd = DirsCmd {
                            sort: sort.into(),
                            cd: true,
                            query: kw,
                            ..Default::default()
                        };
                        return handle_dirs(cli, cmd, cfg).await;
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
            };
            is_default_dir = true;

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
            let vis = Visibility::from_cmd_or_cfg(cmd.vis, cfg.global.panes.nav.default_visibility);

            FsPane::new_nav(
                cwd,
                vis,
                cmd.sort.unwrap_or(cfg.global.panes.nav.default_sort),
            )
        } else
        // interactively search the best match
        {
            FsPane::new_fd_from_command(
                cmd,
                is_default_dir,
                cfg.global.panes.find.default_visibility,
                cwd,
            )
        }
    } else if
    // any fd arg is specified
    !cmd.paths.is_empty()
        || !cmd.types.is_empty()
        || !cmd.vis.is_default()
        || !cmd.fd.is_empty()
    {
        if cmd.vis.is_default() {
            cmd.vis = cfg.global.panes.find.default_visibility
        }

        // pattern specified
        let cwd = if cmd.paths.len() == 1 {
            is_default_dir = true;
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

        let pool_clone = pool.clone();
        tokio::spawn(async move {
            if let Ok(mut conn) = pool_clone.get_conn(DbTable::dirs).await {
                for path in paths {
                    conn.bump(AbsPath::new(path), 1).await._elog();
                }
            }
        });

        if cmd.list {
            // mirror new_fd behavior
            let mut vis =
                Visibility::from_cmd_or_cfg(cmd.vis, cfg.global.panes.find.default_visibility);
            if cmd.vis.hidden.is_none() && auto_enable_hidden(&cmd.paths) {
                vis.hidden = true;
            }

            let (prog, args) = (
                "fd",
                build_fd_args(vis, &cmd.types, &cmd.paths, &cmd.fd, &cfg.global.fd),
            );

            let stdout = match Command::new(prog).args(args).spawn_piped()._ebog() {
                Some(s) => s,
                None => return Err(CliError::Handled),
            };

            let template = EnvOpts::with_env(|s| s.output_template.clone());
            let output_separator =
                EnvOpts::with_env(|s| s.output_separator.clone()).unwrap_or("\n".into());

            let _ = map_reader_lines::<true, CliError>(stdout, move |line| {
                let path = PathBuf::from(line);
                let push = vis.post_fd_filter(&path);

                if push {
                    print(&path, &template, &output_separator)
                }
                Ok(())
            });
            return Ok(());
        };

        FsPane::new_fd_from_command(
            cmd,
            is_default_dir,
            cfg.global.panes.find.default_visibility,
            cwd,
        )
    } else {
        let DefaultCommand { sort, .. } = cmd;
        let vis = Visibility::from_cmd_or_cfg(cmd.vis, cfg.global.panes.nav.default_visibility);

        if cmd.list {
            let iter = list_dir(__cwd(), vis, 1); // cwd is abs so we can add results as unchecked
            let sort = sort.unwrap_or_default();
            let template = EnvOpts::with_env(|s| s.output_template.clone());
            let output_separator =
                EnvOpts::with_env(|s| s.output_separator.clone()).unwrap_or("\n".into());

            match sort {
                SortOrder::none => {
                    for path in iter {
                        print(&path, &template, &output_separator)
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
                        print(&path, &template, &output_separator)
                    }
                }
            }
            return Ok(());
        };

        let vis = Visibility::from_cmd_or_cfg(cmd.vis, cfg.global.panes.nav.default_visibility);
        FsPane::new_nav(
            AbsPath::new_unchecked(__cwd()),
            vis,
            sort.unwrap_or(cfg.global.panes.nav.default_sort),
        )
    };

    let mm_cfg = get_mm_cfg(&cli.mm_config, &cfg);
    start(pane, cfg, mm_cfg, pool, cli.enter_prompt).await
}

async fn handle_tools(
    cli: CliOpts,
    ToolsCmd { tool, args, .. }: ToolsCmd,
    cfg: Config,
) -> Result<(), CliError> {
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
            display_ratatui_styles()?;
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

            let lcfg: LessfilterConfig =
                load_type_or_default(lessfilter_cfg_path(), |s| toml::from_str(s));

            let mut handle = if lcfg.settings.tracked_presets.contains(&cmd.preset) {
                let paths = cmd
                    .paths
                    .clone()
                    .into_iter()
                    .filter_map(|path| path.exists().then_some(AbsPath::new(path)));

                Some(tokio::spawn(async move {
                    let pool = Pool::new(cfg.db_path()).await?;
                    let mut conn = pool.get_conn(DbTable::files).await?;
                    conn.push_files_and_folders(paths).await?;
                    Ok::<_, DbError>(())
                }))
            } else {
                None
            };
            if !cmd.no_exec
                && let Some(h) = handle.take()
            {
                let _ = h.await;
            };

            let code = lessfilter::handle(cmd, lcfg);

            if let Some(h) = handle {
                let _ = h.await;
            };

            exit(code)
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

            if reset {
                if let Some(table) = table {
                    let mut conn = Pool::new(cfg.db_path()).await?.get_conn(table).await?;
                    conn.reset_table().await?;
                    _ibog!("Deleted {table}");
                } else {
                    match std::fs::remove_file(cfg.db_path()) {
                        Ok(()) => _ibog!("Deleted {}", cfg.db_path().to_string_lossy()),
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
                        _ibog!("Removed {}.", msg);
                    }
                } else {
                    conn.push_files_and_folders(entry_queue).await?;
                }
            } else {
                let table = table.unwrap_or(table.unwrap_or(DbTable::dirs));
                // glob is per-table
                let mut conn = Pool::new(cfg.db_path()).await?.get_conn(table).await?;

                let glob = GlobBuilder::new(&pattern.unwrap())
                    .build()
                    .__ebog()
                    .compile_matcher();

                let mut to_remove = Vec::new();

                let db_filter = DbFilter::new(&cfg.history).with_resolve_symlinks(table);
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
                    _ibog!("Removed {removed_count} entries.");
                } else {
                    _ibog!("Matched {matched} paths.");
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

pub fn print(
    path: &std::path::Path,
    template: &Option<String>,
    output_separator: &str,
) {
    let mut display = if let Some(template) = &template {
        format_path(template, &AbsPath::new(path))
    } else {
        path.to_string_lossy().into()
    };
    display.push_str(output_separator);
    print!("{display}")
}
