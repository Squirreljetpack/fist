use std::ffi::OsString;

use cba::{_trace, bog::BogOkExt, prints};
use fist_types::filters::SortOrder;
use matchmaker::{
    AttachmentFormatter, Either, MatchError, Matchmaker, PickOptions,
    config::{RenderConfig, TerminalConfig, UiConfig},
    event::EventLoop,
    message::Event,
    nucleo::{Column, Text, Worker, injector::WorkerInjector},
    preview::AppendOnly,
};

use fist_types::When;

use crate::run::state::GLOBAL::db;
use crate::{
    aliases::MMState,
    cli::CliOpts,
    clipboard,
    config::Config,
    db::{DbTable, Pool},
    display::{display_epoch, human_size},
    errors::CliError,
    mount,
    run::{
        action::{fsaction_aliaser, fsaction_handler},
        item::PathItem,
        mm_config::{MATCHER_CONFIG, MMConfig},
        pane::FsPane,
        previewer::make_previewer,
        query_prompt,
        register::{MMExt, emit_print, paste_handler, query_handler, sync_handler},
        reload::{fs_post_reload_new, sync_mode},
        state::{
            AcceptAsPrint, DB_FILTER, GLOBAL, HideMetadata, LockPromptSetting, MENU_ACTIONS, STACK,
            STORE, SmartGitVisibility, TASKS, context::ActionContext, sort, ui::global_ui_init,
        },
    },
    spawn::{Program, open_wrapped},
    ui::{
        confirm_overlay::ConfirmOverlay,
        menu_overlay::MenuOverlay,
        mount_overlay::MountOverlay,
        options_overlay::OptionsOverlay,
        queue_overlay::{AppOverlay, QueueOverlay},
    },
    unzip,
    utils::formatter::preview_formatter,
    watcher::FsWatcher,
};

pub type FsInjector = WorkerInjector<PathItem>;
pub type FsMatchmaker = Matchmaker<PathItem, PathItem>;

// todo: prompt needs to show initial fs pattern if given
fn make_mm(
    render: RenderConfig,
    tui: TerminalConfig,
    template: Option<String>,
    separator: String,
    print_handle: AppendOnly<String>,
) -> (FsMatchmaker, FsInjector) {
    let worker = Worker::new(
        [
            Column::new("path", |item: &PathItem, _d: &()| {
                if let Ok([_, o]) = &item.tail
                    && !o.is_empty()
                {
                    return Text::from(o.clone());
                }
                item.render()
            })
            .with_raw(|item: &PathItem, _d: &()| item.match_name()),
            Column::new("", |item: &PathItem, _| format_tail(item))
                .with_raw(|item: &PathItem, _d: &()| item.tail_raw())
                .highlight_if(|item: &PathItem, _: &()| !tail_shows_metadata(item)),
            Column::new("3", |item: &PathItem, _d: &()| {
                // rg panes only: there the sort is none and metadata holds the packed loc
                if sort::order_is_none() && item.value() != u64::MAX {
                    Text::from(item.loc().0.to_string())
                } else {
                    Text::from("")
                }
            })
            .without_filtering(),
        ],
        0,
    );
    // the engaged sort is applied by sort::set_sort_from_pane
    // (start initializer + fs_reload), not here at worker construction

    let injector = worker.injector();

    // One closure owns the entire accept decision (matchmaker-cli mm.output
    // pattern): the aliaser sets AcceptAsPrint when the keypress resolves to
    // the print flavor; the hook then emits the selection and returns nothing
    // for the opener/apps to consume. Open flavor = selected-or-current
    // clones, consumption unchanged. Runs on the render thread after
    // tui.exit, so STORE/GLOBAL thread-locals are usable.
    let hook_handle = print_handle.clone();
    let hook_template = template.clone();
    let hook_sep = separator.clone();
    let mut mm = Matchmaker::new(worker, move |state: &mut MMState<'_>| {
        if STORE::take::<AcceptAsPrint>().is_some() {
            // the sink consumes a single item (`:d --cd` prints one path)
            let items: Vec<PathItem> = if GLOBAL::cfg().interface.no_multi_print {
                state.current_raw().cloned().into_iter().collect()
            } else {
                state.map_selected_to_vec(|_, it| it.clone())
            };
            for item in &items {
                db().bump_path(item.path.is_dir(), item.path.clone());
                emit_print(&hook_handle, item, hook_template.as_deref(), &hook_sep);
            }
            return vec![];
        }
        state.map_selected_to_vec(|_, it| it.clone())
    });

    mm.config_render(render);
    // command-output copy needs this after `tui` is consumed below
    let _copy_trailing_newline = tui.copy_trailing_newline;
    mm.config_tui(tui);
    mm.config_matcher(MATCHER_CONFIG);

    // registration order = discriminant ownership order on the shared
    // ExecuteSilent/ExecuteAsync interrupts
    mm.register_print_handler(print_handle, template, separator);
    // attach previewer handling alt-h: help display, display file/fn
    mm.register_become_handler();
    mm.register_execute_handler();
    mm.register_execute_silent_handler();
    mm.register_execute_async_handler();
    mm.register_reload_handler();

    mm.register_event_handler(Event::Synced, sync_handler);
    mm.register_event_handler(Event::QueryChange, query_handler);

    // cwd prompt <=> cursor_disabled can change on any cursor movement
    mm.register_event_handler(Event::CursorChange, |state, _| {
        query_prompt::refresh_prompt(state);
    });

    (mm, injector)
}

/// Column 2 display: the sort value (mtime/atime/size) when hard-sorted with
/// an empty tail, else the tail text.
///
/// The metadata override is skipped while a fresh sort override is hiding it
/// ([`HideMetadata`], stored by [`crate::run::reload::fs_reload`] and the
/// start initializer when the pane's sort matches its configured `default_sort`,
/// and consumed by `ReSort`): the pane renders the plain tail until the first
/// explicit re-sort.
///
/// mtime/atime values live on the item ([`PathItem::value`], unset =
/// `u64::MAX` → blank). Size is deliberately NOT stored on the item — the
/// sort reads the [`fist_size::DirSizeCache`] directly, so the display must
/// too ([`sort::size_of`]): `None` = dir not computed yet → blank; `Some(0)`
/// = genuinely empty file/dir → "0 B".
/// Whether the second column is showing sort metadata instead of its filter
/// text. In that state the displayed cell is not the injected tail, so it must
/// not receive match highlights.
fn tail_shows_metadata(item: &PathItem) -> bool {
    let order = sort::get_sort().order;
    matches!(
        order,
        SortOrder::mtime | SortOrder::atime | SortOrder::size
    ) && matches!(&item.tail, Ok([s, _]) if s.is_empty())
        // a fresh sort override hides the metadata override until the
        // first explicit re-sort (ReSort takes the marker)
        && STORE::get::<HideMetadata>().is_none()
}

fn format_tail(item: &PathItem) -> Text<'static> {
    let order = sort::get_sort().order;
    if tail_shows_metadata(item) {
        return match order {
            SortOrder::mtime | SortOrder::atime => {
                let v = item.value();
                if v == u64::MAX {
                    // unset — render nothing
                    return Text::from("");
                }
                Text::from(display_epoch(v as i64))
            }
            SortOrder::size => match sort::size_of(&item.path) {
                Some(v) => Text::from(human_size(v, true)),
                None => Text::from(""), // not in cache yet (dir pre-fill)
            },
            SortOrder::name | SortOrder::none => unreachable!(),
        };
    }
    item.tail_text()
}

// entrypoint
pub async fn start(
    pane: FsPane,
    cfg: Config,
    mm_cfg: MMConfig,
    db_pool: Pool,
    cli: CliOpts,
) -> Result<(), CliError> {
    let CliOpts {
        lock_prompt,
        output,
        ..
    } = cli;

    // init configs
    let MMConfig {
        previewer,
        render,
        binds,
        queue,
        app,
        options,
        prompt,
        menu,
        confirm,
        mount,
        tui,
        overlay,
        help,
        query_locked,
    } = mm_cfg;
    _trace!(cfg);

    use matchmaker_partial::Apply;

    let mut locked_render = render.clone();
    locked_render.query.apply(query_locked.query);
    locked_render.status.apply(query_locked.status);
    STORE::set(locked_render);

    let print_handle = AppendOnly::new();
    let UiConfig {
        tick_rate,
        mouse_events,
        mouse_scroll_debounce_ms,
        ..
    } = render.ui;

    let osc52 = tui.osc52;
    let copy_trailing_newline = tui.copy_trailing_newline;

    // init MM
    let (mut mm, injector) = make_mm(
        render,
        tui,
        output.format.clone(),
        output.output_sep.clone().unwrap_or_else(|| "\n".into()),
        print_handle.clone(),
    );

    let event_loop = EventLoop::with_binds(binds)
        .with_tick_rate(tick_rate)
        .with_mouse_events(mouse_events)
        .with_scroll_debounce(mouse_scroll_debounce_ms);

    let bind_tx = event_loop.bind_controller();

    // init previewer
    let formatter: AttachmentFormatter<PathItem, ()> = Either::Right(preview_formatter);
    let binds_ptr = event_loop.get_binds_ptr();
    let previewer = make_previewer(
        &mut mm,
        previewer,
        formatter,
        Box::new({
            let binds_ptr = binds_ptr.clone();
            move |config| matchmaker::binds::display_help(&binds_ptr.load(), config)
        }),
    );
    let help_binds = binds_ptr.clone();
    mm.register_help_handler(
        Box::new(move |config| matchmaker::binds::display_help(&binds_ptr.load(), config)),
        help,
        help_binds,
    );

    let mut context = ActionContext::new(print_handle.clone());

    // configure mm
    let mut builder = PickOptions::new()
        .previewer(previewer)
        .event_loop(event_loop)
        .ext_handler(move |x, y| fsaction_handler(x, y, &mut context))
        .ext_aliaser(fsaction_aliaser)
        .initializer(move |state| {
            sort::set_sort_from_pane(state, false);
            // the sort fn only governs the next match pass, and populate's
            // first pass already ran before this initializer: re-sort the
            // matches so a pane created with an explicit sort starts sorted
            state.worker_resort();
            fs_post_reload_new(state);
            if let Some(enter) = lock_prompt {
                STORE::set(LockPromptSetting(enter));
                match enter {
                    When::Always => query_prompt::lock_prompt(state, true),
                    When::Never => query_prompt::lock_prompt(state, false),
                    When::Auto => {
                        let is_nonempty = !state.picker_ui.query.input().is_empty();
                        query_prompt::lock_prompt(state, is_nonempty);
                    }
                }
            };
            query_prompt::refresh_prompt(state); // defensive
        })
        .paste_handler(paste_handler)
        .overlay_config(overlay)
        .overlay(QueueOverlay::new(queue.clone()))
        .overlay(AppOverlay::new(app))
        .overlay(OptionsOverlay::new(options))
        .overlay(ConfirmOverlay::new(confirm))
        .overlay(MenuOverlay::new(
            menu,
            prompt,
            MENU_ACTIONS
                .get()
                .expect("MENU_ACTIONS must be set before start")
                .clone(),
        ))
        .overlay(MountOverlay::new(mount));

    let render_tx = builder.render_tx();

    // start fs-watcher
    let (watcher, watcher_tx) = FsWatcher::new(cfg.notify, render_tx.clone());

    // init history capabilities
    DB_FILTER
        .set(cfg.history.clone())
        .expect("DB_FILTER initialized more than once");
    GLOBAL::init(cfg.global, render_tx, watcher_tx, db_pool, pane, bind_tx);
    sync_mode();
    if GLOBAL::cfg().smart_visibility.git_repo == When::Always {
        STORE::set(SmartGitVisibility);
    }
    if STACK::with_current(FsPane::on_default_sort) {
        STORE::set(HideMetadata);
    }
    sort::set_global_sort_from_pane(false);
    clipboard::init(osc52, copy_trailing_newline);
    crate::spawn::init_spawn_with(cfg.misc.spawn_with);
    global_ui_init(cfg.styles);

    // start watcher
    watcher.spawn()._ebog();

    // populate mm
    STACK::populate(injector, || {});

    // run and wait for mm
    let ret = mm.pick(builder).await;
    // print before errors
    print_handle.map_to_vec(|s| prints!(s));

    // Stop in-flight transfers under TASKS so any delay is surfaced to the
    // user. The cleanup task waits for them, then ejects this process's
    // mounts and removes the extraction root; both tasks are tracked, so the
    // warn/max deadline (and its logging) covers a detach that bogs down.
    let (transfers_done_tx, transfers_done_rx) = tokio::sync::oneshot::channel::<()>();
    TASKS::spawn_blocking("stopping background transfers", move || {
        crate::run::queue::shutdown(std::time::Duration::from_secs(3));
        let _ = transfers_done_tx.send(());
    });
    TASKS::spawn_blocking(
        "ejecting volumes and cleaning temporary directories",
        move || {
            let _ = transfers_done_rx.blocking_recv();
            unzip::shutdown();
            mount::shutdown();
        },
    );
    TASKS::shutdown(400, 10, 3000).await;
    // In the app pane, the picked program opens the pane's pending files
    // (fs :open, the OpenWith menu action); elsewhere the picked lines are
    // paths to open.
    let app_files: Option<Vec<OsString>> = STACK::with_current(|p| match p {
        FsPane::Apps { pending, .. } => Some(
            pending
                .iter()
                .map(|p| p.inner().as_os_str().to_os_string())
                .collect(),
        ),
        _ => None,
    });
    if let Some(files) = app_files {
        match ret {
            Ok(lines) if !lines.is_empty() => {
                let prog = &lines[0];
                let mut conn = db().get_conn(DbTable::apps).await?;
                let cmd = conn.get_cmd(&prog.path).await?;

                let prog = Program::from_scanned_path(prog.path.clone(), cmd);

                open_wrapped(Some(conn), Some(prog), &files, true).await?;
                Ok(())
            }
            Ok(_) => Ok(()),
            Err(e) => Err(e.into()),
        }
    } else {
        match ret {
            Ok(lines) if lines.is_empty() => Ok(()),
            Ok(lines) => {
                set_envs(&lines);

                let files: Vec<OsString> = lines
                    .iter()
                    .map(|p| OsString::from(p.path.inner()))
                    .collect();
                let conn = db().get_conn(DbTable::apps).await?;
                let (opener, detach) = output.opener_parts();
                let prog = opener.and_then(Program::from_os_string);
                if prog.is_some() {
                    crate::spawn::init_spawn_with(Vec::new()); // if opener is set explicitly, ignore spawn_with
                }

                // the default is the same behavior as fs :open, which also called by fs :tool lessfilter open
                open_wrapped(Some(conn), prog, &files, detach).await?;
                Ok(())
            }
            Err(MatchError::Abort(i)) => std::process::exit(i),
            Err(e) => Err(e.into()),
        }
    }
}

fn set_envs(lines: &[PathItem]) {
    let envs = STACK::with_current(|x| match x {
        FsPane::Search { .. } => {
            if lines.len() > 1 {
                return None;
            }
            let (line, col) = lines[0].loc();
            Some((line as usize, (col != 0).then_some(col as usize)))
        }
        _ => None,
    });

    if let Some((line, maybe_col)) = envs {
        unsafe {
            std::env::set_var("HIGHLIGHT_LINE", line.to_string());
            if let Some(c) = maybe_col {
                std::env::set_var("HIGHLIGHT_COLUMN", c.to_string());
            }
        }
    }
}
