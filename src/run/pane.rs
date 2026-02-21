use std::{
    ffi::{OsStr, OsString},
    fmt::Display,
    io::{self, Read},
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::bail;
use cli_boilerplate_automation::{
    bait::ResultExt,
    bath::PathExt,
    bo::{MapReaderError, map_reader_lines},
    bog::BogOkExt,
    broc::{CommandExt, display_sh_prog_and_args},
    bs::sort_by_mtime,
    unwrap,
};
use matchmaker::{message::RenderCommand, nucleo::injector::Injector, preview::AppendOnly};
use tokio::task::spawn_blocking;

use crate::{
    abspath::AbsPath,
    cli::DefaultCommand,
    db::{DbSortOrder, DbTable},
    filters::{SortOrder, Visibility},
    find::{
        FileTypeArg,
        apps::collect_apps,
        fd::{auto_enable_hidden, build_fd_args},
        walker::list_dir,
    },
    run::{
        FsAction,
        item::PathItem,
        start::FsInjector,
        state::{APP, GLOBAL, STACK},
    },
    utils::text::split_delim,
};
use crate::{config::GlobalConfig, utils::size::sort_by_size};

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
        mut cmd: DefaultCommand,
        cwd: AbsPath,
    ) -> Self {
        if auto_enable_hidden(&cmd.paths) {
            cmd.vis.hidden = true;
        }

        let DefaultCommand {
            sort,
            vis,
            types,
            paths,
            fd,
            ..
        } = cmd;

        Self::Fd {
            cwd,
            complete: Default::default(),
            input: Default::default(), // probably will be filled later
            sort: sort.unwrap_or_default(),
            vis,
            types,
            paths,
            fd_args: fd,
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
            vis: vis.validated(),
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
            vis: vis.validated(),
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
                input: (String::new(), 0),
            }
        } else {
            Self::Files {
                sort,
                input: (String::new(), 0),
            }
        }
    }

    #[inline]
    pub fn supports_sort(&self) -> bool {
        matches!(
            self,
            FsPane::Nav { .. } | FsPane::Custom { .. } | FsPane::Fd { .. }
        )
    }

    #[inline]
    pub fn stability_threshold(&self) -> u32 {
        // 0 -> always sort
        match self {
            FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Launch { .. } => 5,
            FsPane::Custom { .. } | FsPane::Stream { .. } => 5, // maybe
            _ => 0,
        }
    }

    #[inline]
    pub fn should_cancel_input_entering_dir(&self) -> bool {
        true
        // todo: allow customizing?
        // matches!(self, FsPane::Nav { .. } | FsPane::Launch { .. })
    }

    pub fn get_input(&self) -> String {
        match self {
            FsPane::Custom { input, .. }
            | FsPane::Stream { input, .. }
            | FsPane::Fd { input, .. }
            | FsPane::Rg { input, .. }
            | FsPane::Nav { input, .. }
            | FsPane::Files { input, .. }
            | FsPane::Folders { input, .. } => input.0.clone(),
            _ => String::new(),
        }
    }
}

pub static STOP: AtomicBool = AtomicBool::new(false);

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
                let cwd = cwd.clone();
                let items = items.clone();

                log::info!("spawning: {}", display_sh_prog_and_args(prog, args));

                let stdout = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;

                let vis = *vis;

                let sem = Arc::new(tokio::sync::Semaphore::new(GLOBAL::with_cfg(|c| {
                    c.panes.settings.display_script_simultaneous_count
                })));

                let batch_collect_size =
                    2 * GLOBAL::with_cfg(|c| c.panes.settings.display_script_batch_size);
                let mut batch_collect = Vec::with_capacity(batch_collect_size);

                match display_script {
                    None => map_reader(
                        stdout,
                        move |line| {
                            let item = PathItem::new_from_split(split_delim(&line, delim), &cwd);
                            if vis.filter(&item.path) {
                                injector.push(item)?;
                            }
                            anyhow::Ok(())
                        },
                        complete.clone(),
                        true,
                    ),
                    Some(Ok(script)) => {
                        // Script runs per item asynchronously
                        map_reader(
                            stdout,
                            move |line| {
                                if STOP.load(Ordering::SeqCst) {
                                    bail!("Canceled");
                                }
                                let injector = injector.clone();
                                let sem = sem.clone();
                                let cwd = cwd.clone();
                                let script = script.clone();

                                tokio::spawn(async move {
                                    let _permit = sem.acquire_owned().await.unwrap();
                                    if let Ok(out) =
                                        crate::spawn::utils::tokio_command_from_script(&script)
                                            .args(split_delim(&line, delim))
                                            .output()
                                            .await
                                    {
                                        let mut item = PathItem::new_from_split(
                                            split_delim(&line, delim),
                                            &cwd,
                                        );
                                        if out.status.success() {
                                            if let Ok(rendered) =
                                                ansi_to_tui::IntoText::into_text(&out.stdout)
                                            {
                                                item.override_rendered(rendered);
                                            }
                                        }
                                        if injector.push(item).is_err() {
                                            STOP.store(true, Ordering::SeqCst);
                                        }
                                    }
                                });
                                Ok(())
                            },
                            complete.clone(),
                            true,
                        )
                    }
                    Some(Err(script)) => map_reader(
                        stdout,
                        move |line| {
                            if STOP.load(Ordering::SeqCst) {
                                bail!("Canceled");
                            }
                            let [p1, p2] = split_delim(&line, delim);
                            batch_collect.push([p1.to_string(), p2.to_string()]);

                            if batch_collect.len() >= batch_collect_size {
                                let batch = std::mem::take(&mut batch_collect);
                                let batch_count = batch.len();
                                let injector = injector.clone();
                                let cwd = cwd.clone();
                                let script = script.clone();

                                tokio::task::spawn_blocking(move || {
                                    if let Some(stdout) = Command::from_script(&script)
                                        .args(batch.iter().flatten())
                                        .current_dir(&cwd)
                                        .spawn_piped()
                                        ._ebog()
                                    {
                                        let mut batch_iter = batch.into_iter();

                                        match map_reader_lines::<true, ()>(stdout, move |line| {
                                            let [p1, p2] = unwrap!(batch_iter.next(); ());
                                            {
                                                let mut item =
                                                    PathItem::new_from_split([&p1, &p2], &cwd);

                                                if let Ok(rendered) =
                                                    ansi_to_tui::IntoText::into_text(&line)
                                                {
                                                    item.override_rendered(rendered);
                                                };

                                                injector
                                                    .push(item)
                                                    .map_err(|_| STOP.store(true, Ordering::SeqCst))
                                            }
                                        }) {
                                            Ok(n) if n < batch_count => {
                                                log::warn!(
                                                    "Items dropped while processing display-batch: Insufficient lines"
                                                )
                                            }
                                            Err(MapReaderError::ChunkError(x, y)) => {
                                                log::error!(
                                                    "Error while processing display-batch: Failed to read chunk {x}: {y}"
                                                )
                                            }
                                            _ => {}
                                        }
                                    } else if injector
                                        .extend(batch.into_iter().map(|[p1, p2]| {
                                            PathItem::new_from_split([&p1, &p2], &cwd)
                                        }))
                                        .is_err()
                                    {
                                        STOP.store(true, Ordering::SeqCst);
                                    }
                                });
                            }

                            Ok(())
                        },
                        complete.clone(),
                        true,
                    ),
                }
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

                let items = items.clone();
                items.map_to_vec(|item| injector.push(item.clone())); // stdin reads resume

                let cwd = cwd.clone();
                let vis = *vis;
                let sem = Arc::new(tokio::sync::Semaphore::new(GLOBAL::with_cfg(|c| {
                    c.panes.settings.display_script_simultaneous_count
                })));

                map_reader(
                    io::stdin(),
                    move |line| {
                        let mut item = PathItem::new_from_split(split_delim(&line, delim), &cwd);

                        if !vis.filter(&item.path) {
                            return Ok(());
                        }
                        // apply render override
                        if let Some(Ok(script)) = display_script.clone() {
                            let injector = injector.clone();
                            let sem = sem.clone();
                            tokio::spawn(async move {
                                let _permit = sem.acquire_owned().await.unwrap();
                                if let Ok(out) =
                                    crate::spawn::utils::tokio_command_from_script(&script)
                                        .arg(&item.path)
                                        .arg(OsStr::new(
                                            item.tail.lines[0].spans[0].content.as_ref(),
                                        ))
                                        // .stderr(Stdio::null())
                                        .output()
                                        .await
                                {
                                    if out.status.success()
                                        && let Ok(rendered) =
                                            ansi_to_tui::IntoText::into_text(&out.stdout)
                                    {
                                        item.override_rendered(rendered);
                                    }
                                    injector.push(item)
                                } else {
                                    Ok(())
                                }
                            });
                            Ok(())
                        } else {
                            injector.push(item)
                        }
                    },
                    complete.clone(),
                    false,
                )
            }

            Self::Fd {
                cwd,
                complete,
                // input,
                // sort,
                vis,
                types,
                paths,
                fd_args,
                ..
            } => {
                let vis = *vis;
                let cwd = cwd.clone();
                let (prog, args) = ("fd", build_fd_args(vis, types, paths, fd_args, &cfg.fd));

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
                    STACK::len() == 1,
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
                let vis = *vis;
                let cwd = cwd.clone();
                let (prog, args) = ("fd", build_fd_args(vis, types, paths, fd_args, &cfg.fd));

                log::info!("spawning: {}", display_sh_prog_and_args(prog, &args));

                let stdout = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;

                // Example output of rg 'command' --case-sensitive -C 2
                // node_modules/@babel/core/lib/parser/util/missing-plugin-helper.js
                // 328-You can re-run Babel with the BABEL_SHOW_CONFIG_FOR environment variable to show the loaded \
                // 329-configuration:
                // 330:\tnpx cross-env BABEL_SHOW_CONFIG_FOR=${msgFilename} <your build command>
                // 331-See https://babeljs.io/docs/configuration#print-effective-configs for more info.
                // 332-`;
                // 89-      * accounting for any retained ranges.
                // 90-      * @param {SourceRange} range The range to remove in the fix.
                // 91-      * @param {string} text The text to insert in place of the range.
                // 92:      * @returns {Object} The fix command.
                // 93-      */
                // 94-     replaceTextRange(range, text) {
                // --
                // 113-
                // 114-    /**
                // 115:     * Create a fix command that removes the given node or token, accounting for
                // 116-     * any retained ranges.
                // 117-     * @param {ASTNode|Token} nodeOrToken The node or token to remove.
                // 118:     * @returns {Object} The fix command.
                // 119-     */
                // 120-    remove(nodeOrToken) {
                //
                // node_modules/@eslint-community/eslint-utils/node_modules/eslint-visitor-keys/README.md
                // 94-Welcome. See [ESLint contribution guidelines](https://eslint.org/docs/developer-guide/contributing/).
                // 95-
                // 96:### Development commands
                // 97-
                // 98-- `npm test` runs tests and measures code coverage.
                //
                // node_modules/@babel/parser/CHANGELOG.md
                // 89- * Changed Non-existent RestPattern to RestElement which is what is actually parsed (#409) [skip ci] (James Browning)
                // 90- * Upgrade flow to 0.41 (Daniel Tschinder)
                // 91: * Fix watch command (#403) (Brian Ng)
                // 92- * Update yarn lock (Daniel Tschinder)
                // 93: * Fix watch command (#403) (Brian Ng)
                // 94- * chore(package): update flow-bin to version 0.41.0 (#395) (greenkeeper[bot])
                // 95- * Add estree test for correct order of directives (Daniel Tschinder)
                // --
                // 487-![image](https://cloud.githubusercontent.com/assets/5233399/19420267/388f556e-93ad-11e6-813e-7c5c396be322.png)
                // 488-
                // 489:- add clean command [skip ci] ([#201](https://github.com/babel/babylon/pull/201)) (Henry Zhu)
                // 490-- add ForAwaitStatement (async generator already added) [skip ci] ([#196](https://github.com/babel/babylon/pull/196)) (Henry Zhu)
                // 491-

                // with --no-heading:
                // --
                // node_modules/@babel/parser/CHANGELOG.md-487-![image](https://cloud.githubusercontent.com/assets/5233399/19420267/388f556e-93ad-11e6-813e-7c5c396be322.png)
                // node_modules/@babel/parser/CHANGELOG.md-488-
                // --

                // So : => present, - => context
                // empty line => next line is path
                // -- => context break

                // haven't yet tested multiline
                // currently, we take the easiest approach of having a seperate item for each context block

                // let mut current_path = String::new();
                // let mut current_context = vec![];
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
                    STACK::len() == 1,
                )
            }
            Self::Files { sort, .. } => {
                let sort = *sort;
                let cwd = STACK::cwd().unwrap_or_default();
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::files).await.elog()?;
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?.into_iter();

                    for e in entries {
                        let item = PathItem::new_unchecked(e.path.into(), &cwd);
                        injector.push(item)?;
                    }

                    Ok(())
                })
            }
            Self::Folders { sort, .. } => {
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
                        if conn.create_many(&entries).await? > 0 {
                            GLOBAL::send_action(FsAction::Reload);
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

                spawn_blocking(move || {
                    let iter = list_dir(&cwd, vis, depth); // cwd is abs so we can add results as unchecked

                    match sort {
                        SortOrder::none => {
                            for path in iter {
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
                                SortOrder::size => sort_by_size(&mut files),
                                SortOrder::none => unreachable!(),
                            }

                            for path in files.into_iter() {
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
    abort_empty: bool,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    spawn_blocking(move || {
        let count = map_reader_lines::<true, E>(reader, f)._elog();
        match count {
            Some(0) if abort_empty => {
                GLOBAL::send_mm(RenderCommand::QuitEmpty);
            }
            _ => {}
        }
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
