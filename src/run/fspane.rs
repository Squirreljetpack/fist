use std::{
    ffi::OsString,
    fmt::Display,
    io::{self, Read},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use cli_boilerplate_automation::{
    bait::ResultExt,
    bath::PathExt,
    bo::map_reader_lines,
    bog::BogOkExt,
    broc::{CommandExt, display_sh_prog_and_args},
    bs::sort_by_mtime,
};
use matchmaker::{efx, nucleo::injector::Injector, preview::AppendOnly, render::Effect};

use crate::config::GlobalConfig;
use crate::{
    abspath::AbsPath,
    cli::DefaultCommand,
    db::{DbSortOrder, DbTable},
    filters::{SortOrder, Visibility},
    find::{
        apps::collect_apps,
        fd::{FileTypeArg, build_fd_args},
        walker::list_dir,
    },
    run::{
        globals::APP,
        item::PathItem,
        start::FsInjector,
        state::{GLOBAL, STACK},
    },
};

#[derive(Debug, Clone)]
pub enum FsPane {
    Custom {
        cwd: AbsPath,
        items: AppendOnly<PathItem>, // concurrency
        cmd: (OsString, Vec<OsString>),
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        // experimental
        sort: SortOrder,
        vis: Visibility,
    },
    Stream {
        cwd: AbsPath,
        items: AppendOnly<PathItem>, // concurrency
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        // experimental
        sort: SortOrder,
        vis: Visibility,
    },
    Fd {
        cwd: AbsPath,
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        sort: SortOrder,
        vis: Visibility,
        types: Vec<FileTypeArg>,
        paths: Vec<OsString>,
        fd_args: Vec<OsString>,
    },
    Rg {
        cwd: AbsPath,
        complete: Arc<AtomicBool>,
        input: (String, u32), // input, INDEX

        vis: Visibility,
        types: Vec<FileTypeArg>,
        paths: Vec<OsString>,
        fd_args: Vec<OsString>,
    },
    Files {
        sort: DbSortOrder,
        input: (String, u32), // input, INDEX
    },
    Folders {
        sort: DbSortOrder,
        fd_args: Vec<OsString>,
        input: (String, u32), // input, INDEX
    },
    Launch {
        sort: DbSortOrder,
        complete: Arc<AtomicBool>,
    },
    Nav {
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
        input: (String, u32), // input, INDEX
        complete: Arc<AtomicBool>,
        depth: usize,
    },
}

impl FsPane {
    /// Converts cwd to normalized absolute and stores it
    /// Executes cmd, otherwise populates from stdin
    pub fn new_custom(
        cwd: AbsPath,
        visibility: Visibility,
        cmd: (OsString, Vec<OsString>),
    ) -> Self {
        Self::Custom {
            cwd,
            items: Default::default(),
            cmd,
            vis: visibility,
            sort: SortOrder::none,
            complete: Default::default(),
            input: Default::default(),
        }
    }

    pub fn new_launch() -> Self {
        Self::Launch {
            sort: DbSortOrder::frecency,
            complete: Default::default(),
        }
    }

    pub fn new_stream(
        cwd: AbsPath,
        visibility: Visibility,
    ) -> Self {
        Self::Stream {
            cwd,
            items: Default::default(),
            vis: visibility,
            sort: SortOrder::none,
            complete: Default::default(),
            input: Default::default(),
        }
    }

    pub fn new_fd_from_command(
        cmd: DefaultCommand,
        cwd: AbsPath,
    ) -> Self {
        Self::Fd {
            cwd,
            complete: Default::default(),
            input: Default::default(), // probably will be filled later
            sort: cmd.sort.unwrap_or_default(),
            vis: cmd.vis,
            types: cmd.types,
            paths: cmd.paths,
            fd_args: cmd.fd,
        }
    }

    /// Create a fd pane in the current directory
    pub fn new_fd(
        cwd: AbsPath,
        sort: SortOrder,
        vis: Visibility,
    ) -> Self {
        Self::Fd {
            paths: vec![cwd.inner().into(), '.'.to_string().into()],
            cwd,
            complete: Default::default(),
            input: Default::default(), // probably will be filled later
            sort,
            vis: vis.validate(),
            types: Default::default(),
            fd_args: vec![],
        }
    }

    pub fn new_nav(
        cwd: AbsPath,
        vis: Visibility,
        sort: SortOrder,
    ) -> Self {
        Self::Nav {
            cwd,
            sort,
            vis: vis.validate(),
            depth: 1,
            input: Default::default(),
            complete: Default::default(),
        }
    }

    pub fn new_history(
        folders: bool,
        sort: DbSortOrder,
    ) -> Self {
        if folders {
            Self::Folders {
                sort,
                fd_args: vec![],
                input: (String::new(), 0),
            }
        } else {
            Self::Files {
                sort,
                input: (String::new(), 0),
            }
        }
    }

    pub fn supports_sort(&self) -> bool {
        matches!(
            self,
            FsPane::Nav { .. } | FsPane::Custom { .. } | FsPane::Fd { .. }
        )
    }
}

// todo: lowpri: this is like 1.2 slower than pure fd? Accept input is a bit sluggish? Cache to reduce disk reads?
// Doesn't block, uses tokio::spawn
// todo: get rid of _callback
impl FsPane {
    pub fn populate(
        &self,
        injector: FsInjector,
        cfg: &GlobalConfig,
        _callback: impl FnOnce() + 'static + Send + Sync,
    ) -> Option<tokio::task::JoinHandle<anyhow::Result<()>>> {
        log::debug!("Populating: {self:?}");
        let ret = match self {
            Self::Custom {
                cmd: (prog, args),
                items,
                cwd,
                vis,
                sort,
                complete,
                ..
            } => {
                if complete.load(Ordering::Acquire) {
                    items.map_to_vec(|item| injector.push(item.clone()));
                    return None;
                }
                let delim = GLOBAL::with_env(|c| c.delim);
                let display_script = GLOBAL::with_env(|c| c.display.clone());
                // store current sort/vis in global, then reset self

                let cwd = cwd.clone();
                let items = items.clone();

                log::info!("spawning: {}", display_sh_prog_and_args(prog, args));

                let stdout = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;

                let vis = *vis;

                map_reader(
                    stdout,
                    move |line| {
                        let mut item = if let Some(c) = delim {
                            PathItem::new_from_split(line, c, &cwd)
                        } else {
                            PathItem::new(line, &cwd)
                        };

                        let mut push = vis.filter(&item.path);
                        // apply render override
                        if push
                            && let Some(script) = &display_script
                            && let Ok(out) = Command::from_script(script)
                                .arg(&item.path)
                                .stderr(Stdio::null())
                                .output()
                        {
                            push = out.status.success();
                            if push
                                && let Ok(rendered) = ansi_to_tui::IntoText::into_text(&out.stdout)
                            {
                                item.override_rendered(rendered);
                            }
                        };

                        if push { injector.push(item) } else { Ok(()) }
                    },
                    complete.clone(),
                )
            }

            Self::Stream {
                items,
                cwd,
                vis,
                sort,
                complete,
                ..
            } => {
                if complete.load(Ordering::Acquire) {
                    items.map_to_vec(|item| injector.push(item.clone()));
                    return None;
                }
                let delim = GLOBAL::with_env(|c| c.delim);
                let display_script = GLOBAL::with_env(|c| c.display.clone());
                // store current sort/vis in global, then reset self

                let cwd = cwd.clone();
                let vis = *vis;
                let items = items.clone();

                items.map_to_vec(|item| injector.push(item.clone())); // stdin reads resume
                map_reader(
                    io::stdin(),
                    move |line| {
                        let mut item = if let Some(c) = delim {
                            PathItem::new_from_split(line, c, &cwd)
                        } else {
                            PathItem::new(line, &cwd)
                        };

                        let mut push = vis.filter(&item.path);
                        // apply render override
                        if push
                            && let Some(script) = &display_script
                            && let Ok(out) = Command::from_script(script)
                                .arg(&item.path)
                                .stderr(Stdio::null())
                                .output()
                        {
                            push = out.status.success();
                            if push
                                && let Ok(rendered) = ansi_to_tui::IntoText::into_text(&out.stdout)
                            {
                                item.override_rendered(rendered);
                            }
                        };

                        if push { injector.push(item) } else { Ok(()) }
                    },
                    complete.clone(),
                )
            }

            Self::Fd {
                cwd,
                complete,
                input,
                sort,
                vis,
                types,
                paths,
                fd_args,
                ..
            } => {
                let vis = *vis;
                let cwd = cwd.clone();
                let (prog, args) = (
                    "fd",
                    build_fd_args(*sort, vis, types, paths, fd_args, &cfg.fd),
                );

                log::info!("spawning: {}", display_sh_prog_and_args(prog, &args));

                let stdout = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;

                map_reader(
                    stdout,
                    move |line| {
                        let item = PathItem::new(line, &cwd);

                        let mut push = true;
                        // most checks were already handled by fd
                        if vis.hidden_files && item.path.is_hidden() {
                            if vis.dirs {
                                push = item.path.is_dir();
                            } else {
                                push = !item.path.is_dir()
                            }
                        };
                        if !vis.all() {
                            push = item.path.exists()
                        }

                        if push { injector.push(item) } else { Ok(()) }
                    },
                    complete.clone(),
                )
            }
            Self::Rg {
                cwd,
                complete,
                input,
                vis,
                types,
                paths,
                fd_args,
                ..
            } => {
                todo!()
            }
            Self::Files { sort, .. } => {
                let db_path = cfg.db_path();
                let sort = *sort;
                let cwd = STACK::cwd().unwrap_or_default();
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let conn = pool.get_conn(DbTable::files).await.elog()?;
                    // let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;

                    // for e in entries.filter_exists(true) {
                    //     let item = PathItem::new_unchecked(e.path.into(), &cwd);
                    //     injector.push(item);
                    // }

                    Ok(())
                })
            }
            Self::Folders { sort, fd_args, .. } => {
                let db_path = cfg.db_path();
                let sort = *sort;
                let cwd = STACK::cwd().unwrap_or_default();
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::dirs).await.elog()?;
                    let mut entries = GLOBAL::get_db_entries(&mut conn, sort).await?.into_iter();

                    // skip the first cwd item
                    if matches!(sort, DbSortOrder::atime) {
                        if let Some(e) = entries.next()
                            && e.path != cwd
                        {
                            let item = PathItem::new_unchecked(e.path.into(), &cwd);
                            injector.push(item)?
                        }
                    }

                    for e in entries {
                        let item = PathItem::new_unchecked(e.path.into(), &cwd);
                        injector.push(item)?
                    }

                    Ok(())
                })
            }
            Self::Launch { sort, .. } => {
                let db_path = cfg.db_path();
                let sort = *sort;
                let cwd = STACK::cwd().unwrap_or_default();
                let pool = GLOBAL::db();
                let pool_clone = pool.clone();

                let ret = tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::apps).await.elog()?;
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;

                    for e in entries {
                        let item = PathItem::new_app(e);
                        injector.push(item)?
                    }

                    Ok(())
                });
                if APP::RAN_RECACHE
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
                    .is_ok()
                {
                    tokio::spawn(async move {
                        let mut entries = collect_apps();
                        // initial population in order
                        entries.sort_by(|a, b| a.name.cmp(&b.name));
                        let mut conn = pool_clone.get_conn(DbTable::apps).await.elog()?;
                        if conn.create_many(&entries).await? > 0
                            && APP::RAN_RECACHE
                                .compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire)
                                .is_ok()
                        {
                            GLOBAL::send_efx(efx![Effect::Reload]);
                        }
                        anyhow::Ok(())
                    });
                }

                ret
            }
            Self::Nav {
                cwd,
                sort,
                vis,
                depth,
                complete,
                ..
            } => {
                let cwd = cwd.clone();
                let vis = *vis;
                let sort = *sort;
                let depth = *depth;
                let complete = complete.clone();

                tokio::spawn(async move {
                    let iter = list_dir(&cwd, vis, depth); // cwd is abs so we can add results as unchecked

                    match sort {
                        SortOrder::none => {
                            for (i, path) in iter.enumerate() {
                                let item = PathItem::new_unchecked(path, &cwd);
                                injector.push(item)?
                            }
                        }
                        _ => {
                            let mut files: Vec<PathBuf> = iter.collect();

                            match sort {
                                SortOrder::name => {
                                    files.sort_by(|a, b| a.file_name().cmp(&b.file_name()))
                                }
                                SortOrder::mtime => sort_by_mtime(&mut files),
                                _ => unreachable!(),
                            }

                            for (i, path) in files.into_iter().enumerate() {
                                let item = PathItem::new_unchecked(path, &cwd);
                                injector.push(item)?
                            }
                        }
                    }
                    complete.store(true, Ordering::Release);

                    anyhow::Ok(())
                })
            }
        };
        Some(ret)
    }
}

pub fn map_reader<E: matchmaker::SSS + Display>(
    reader: impl Read + matchmaker::SSS,
    f: impl FnMut(String) -> Result<(), E> + matchmaker::SSS,
    complete: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    tokio::spawn(async move {
        map_reader_lines::<true, E>(reader, f)._elog();
        complete.store(true, Ordering::SeqCst);
        log::info!("Command completed");
        anyhow::Ok(())
    })
}

// --------------------BOILERPLATE-------------------------------

impl PartialEq for FsPane {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}
impl Eq for FsPane {}
