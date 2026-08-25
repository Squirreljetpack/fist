#![allow(unused_variables)]

use std::{borrow::Cow, ffi::OsString, sync::Arc};

use cba::{_trace, bog::BogOkExt, prints};
use fist_types::filters::SortOrder;
use matchmaker::{
    Either, MatchError, Matchmaker, PickOptions,
    config::{PreviewerConfig, RenderConfig, TerminalConfig, UiConfig},
    event::EventLoop,
    message::Event,
    nucleo::{Column, Text, Worker, injector::WorkerInjector},
    preview::AppendOnly,
};

use crate::run::state::GLOBAL::db;
use crate::{
    aliases::MMState,
    cli::CliOpts,
    clipboard,
    config::Config,
    db::{DbTable, Pool},
    display::{display_epoch, human_size},
    errors::CliError,
    run::{
        action::{fsaction_aliaser, fsaction_handler},
        ahandlers::{self, fs_post_reload_new, paste_handler},
        item::PathItem,
        mm_config::{MATCHER_CONFIG, MMConfig},
        pane::FsPane,
        previewer::make_previewer,
        register::{MMExt, emit_print, path_formatter, query_handler, sync_handler},
        state::{
            AcceptFlavor, DB_FILTER, GLOBAL, HideMetadata, MENU_ACTIONS, STACK, STORE, TASKS,
            context::ActionContext, sort, ui::global_ui_init,
        },
    },
    spawn::{Program, open_wrapped},
    ui::{
        confirm_overlay::ConfirmOverlay,
        menu_overlay::MenuOverlay,
        options_overlay::OptionsOverlay,
        queue_overlay::{AppOverlay, QueueOverlay},
    },
    unzip,
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
            Column::new("_", |item: &PathItem, d: &()| {
                if let Ok([_, o]) = &item.tail
                    && !o.is_empty()
                {
                    return Text::from(o.clone());
                }
                item.render()
            })
            .with_raw(|item: &PathItem, d: &()| Cow::Owned(item.display_name())),
            Column::new("", |item: &PathItem, _| format_tail(item)).with_raw(
                |item: &PathItem, d: &()| match &item.tail {
                    Ok([s, _]) => Cow::Borrowed(s.as_str()),
                    Err(t) => Cow::Owned(t.to_string()),
                },
            ),
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
    // stability is applied by sort::set_sort_in_nucleo (fs_reload), which knows the
    // engaged mode — the initial call here was redundant

    let injector = worker.injector();

    // One closure owns the entire accept decision (matchmaker-cli mm.output
    // pattern): the aliaser sets AcceptFlavor when the keypress resolves to
    // the print flavor; the hook then emits the selection and returns nothing
    // for the opener/apps to consume. Open flavor = selected-or-current
    // clones, consumption unchanged. Runs on the render thread after
    // tui.exit, so STORE/GLOBAL thread-locals are usable.
    let hook_handle = print_handle.clone();
    let hook_template = template.clone();
    let hook_sep = separator.clone();
    let mut mm = Matchmaker::new(worker, move |state: &mut MMState<'_>| {
        if STORE::take::<AcceptFlavor>().is_some() {
            // respect no_multi_accept when this was aliased from Accept
            let items: Vec<PathItem> =
                if GLOBAL::cfg().interface.alt_accept && GLOBAL::cfg().interface.no_multi_accept {
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
    let copy_trailing_newline = tui.copy_trailing_newline;
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
        ahandlers::refresh_prompt(state);
    });

    (mm, injector)
}

/// Column 2 display: the sort value (mtime/atime/size) when hard-sorted with
/// an empty tail, else the tail text.
///
/// The metadata override is skipped while a fresh sort override is hiding it
/// ([`HideMetadata`], stored by [`crate::run::ahandlers::fs_reload`] and the
/// start initializer when the pane's sort matches its configured `default_sort`,
/// and consumed by `ReSort`): the pane renders the plain tail until the first
/// explicit re-sort.
///
/// mtime/atime values live on the item ([`PathItem::value`], unset =
/// `u64::MAX` → blank). Size is deliberately NOT stored on the item — the
/// sort reads the [`fist_size::DirSizeCache`] directly, so the display must
/// too ([`sort::size_of`]): `None` = dir not computed yet → blank; `Some(0)`
/// = genuinely empty file/dir → "0 B".
fn format_tail(item: &PathItem) -> Text<'static> {
    let order = sort::get_sort().order;
    // gated on an empty tail so the filter string (always the tail) can never
    // mismatch the display
    let show_value = matches!(
        order,
        SortOrder::mtime | SortOrder::atime | SortOrder::size
    ) && matches!(&item.tail, Ok([s, _]) if s.is_empty())
        // a fresh sort override hides the metadata override until the
        // first explicit re-sort (ReSort takes the marker)
        && STORE::get::<HideMetadata>().is_none();
    if show_value {
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

// "entrypoint", called ONCE
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
        render,
        binds,
        queue,
        app,
        options,
        prompt,
        menu,
        confirm,
        tui,
        overlay,
        help: _,
    } = mm_cfg;
    _trace!(cfg);

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
    let formatter = Either::Left(Arc::new(
        Box::new(path_formatter) as matchmaker::RenderFn<PathItem>
    ));
    let binds_ptr = event_loop.get_binds_ptr();
    let previewer = make_previewer(
        &mut mm,
        PreviewerConfig::default(),
        formatter,
        Box::new(move |config| matchmaker::binds::display_help(&binds_ptr.load(), config)),
    );

    let mut context = ActionContext::new(print_handle.clone());

    // configure mm
    let mut builder = PickOptions::new()
        .previewer(previewer)
        .event_loop(event_loop)
        .ext_handler(move |x, y| fsaction_handler(x, y, &mut context))
        .ext_aliaser(fsaction_aliaser)
        .initializer(move |state| {
            sort::set_sort_from_pane(state, true.into());
            state.picker_ui.query.show_border = false;
            fs_post_reload_new(state);
            if let Some(enter) = lock_prompt {
                ahandlers::lock_prompt(state, enter);
            };
            ahandlers::refresh_prompt(state); // defensive
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
        ));

    let render_tx = builder.render_tx();

    // start fs-watcher
    let (watcher, watcher_tx) = FsWatcher::new(cfg.notify, render_tx.clone());

    // init history capabilities
    DB_FILTER
        .set(cfg.history.clone())
        .expect("DB_FILTER initialized more than once");
    // init global
    GLOBAL::init(cfg.global, render_tx, watcher_tx, db_pool, pane, bind_tx);
    sort::set_global_sort_from_pane(true.into());
    clipboard::init(cfg.misc.clipboard_delay_ms, osc52, copy_trailing_newline);
    crate::spawn::init_spawn_with(cfg.misc.spawn_with);
    global_ui_init(cfg.styles);

    // start watcher
    watcher.spawn()._ebog();
    // start the archive extraction worker

    // populate mm
    STACK::populate(injector, || {});

    // run and wait for mm
    let ret = mm.pick(builder).await;
    // print before errors
    print_handle.map_to_vec(|s| prints!(s));

    TASKS::shutdown(500, 10, 3000).await;
    // stop in-flight extractions first so skeleton cleanup is not racing
    // the copy workers
    crate::run::queue::shutdown(std::time::Duration::from_secs(3));
    unzip::shutdown();
    TASKS::shutdown(500, 10, 3000).await;
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

                open_wrapped(conn, Some(prog), &files, true).await?;
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
                let prog = output.opener.as_ref().and_then(Program::from_os_string);
                if prog.is_some() {
                    crate::spawn::init_spawn_with(Vec::new()); // if opener is set explicitly, ignore spawn_with
                }

                // the default is the same behavior as fs :open, which also called by fs :tool lessfilter open
                open_wrapped(conn, prog, &files, false).await?;
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
