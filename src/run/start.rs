use std::{ffi::OsString, sync::Arc};

use cli_boilerplate_automation::{bog::BogOkExt, prints};
use matchmaker::{
    MatchError, MatchResultExt, Matchmaker, PickOptions, RenderFn, Selector, acs,
    action::Action,
    binds::display_binds,
    config::{PreviewerConfig, RenderConfig, TerminalConfig},
    event::EventLoop,
    message::{Event, RenderCommand},
    nucleo::{
        Column, Indexed, Render, Worker,
        injector::{IndexedInjector, WorkerInjector},
    },
    preview::AppendOnly,
    ui::StatusUI,
};
use ratatui::text::Text;

use crate::{
    clipboard,
    config::Config,
    db::{DbTable, Pool, zoxide::DbFilter},
    errors::CliError,
    run::{
        FsAction,
        action::{fsaction_aliaser, fsaction_handler},
        ahandler::paste_handler,
        dhandlers::{MMExt, query_handler, sync_handler},
        item::PathItem,
        mm_config::{MATCHER_CONFIG, MMConfig},
        pane::FsPane,
        previewer::make_previewer,
        state::{APP, DB_FILTER, GLOBAL, STACK, TASKS, context::ActionContext, ui::global_ui_init},
    },
    spawn::{Program, open_wrapped},
    ui::{filters_overlay::FilterOverlay, menu_overlay::MenuOverlay, stash_overlay::StashOverlay},
    watcher::FsWatcher,
};

pub type FsInjector = IndexedInjector<PathItem, WorkerInjector<Indexed<PathItem>>>;
pub type FsMatchmaker = Matchmaker<Indexed<PathItem>, PathItem>;
fn exist_validator(s: &PathItem) -> bool {
    s.path.exists()
}

pub type FormatterFn = Arc<RenderFn<Indexed<PathItem>>>;

// todo: prompt needs to show initial fs pattern if given
fn make_mm(
    render: RenderConfig,
    tui: TerminalConfig,
    _cfg: &Config,
    print_handle: AppendOnly<String>,
    stability: u32,
) -> (Matchmaker<Indexed<PathItem>, PathItem>, FsInjector) {
    let mut worker = Worker::new(
        [
            Column::new("_", |item: &Indexed<PathItem>| item.inner.as_text()),
            Column::new("", |item: &Indexed<PathItem>| item.inner.tail.clone()),
            Column::new("3", |item: &Indexed<PathItem>| {
                Text::from(
                    item.inner
                        .cmd
                        .as_deref()
                        .and_then(|s| s.split_once(':').map(|x| x.0))
                        .unwrap_or_default(),
                )
            })
            .without_filtering(),
        ],
        0,
    );
    worker.set_stability(stability);

    let injector = IndexedInjector::new_globally_indexed(worker.injector());

    let selector = Selector::new(Indexed::identifier).with_validator(exist_validator);
    // todo: we really just want an alternate behavior for accept multi
    // if cfg.global.interface.no_multi {
    //     selector = selector.disabled()
    // }

    let mut mm = Matchmaker::new(worker, selector);

    mm.config_render(render);
    mm.config_tui(tui);

    mm.register_print_handler_(print_handle);
    // attach previewer handling alt-h: help display, display file/fn
    mm.register_become_handler_();
    mm.register_execute_handler_();
    mm.register_reload_handler_();

    mm.register_event_handler(Event::Synced, sync_handler);
    mm.register_event_handler(Event::QueryChange, query_handler);

    (mm, injector)
}

// "entrypoint", called ONCE
pub async fn start(
    pane: FsPane,
    cfg: Config,
    mm_cfg: MMConfig,
    db_pool: Pool,
) -> Result<(), CliError> {
    // init configs
    let MMConfig {
        mut render,
        mut binds,
        stash,
        filters,
        prompt,
        menu,
        tui,
        overlay,
    } = mm_cfg;
    log::debug!("cfg: {cfg:?}");

    if let Some(x) = cfg.global.panes.preview_show(&pane) {
        render.preview.show = x
    }
    if let Some(x) = cfg.global.panes.prompt(&pane) {
        render.input.prompt = x;
    }
    let preview_layout_index = cfg.global.panes.preview_layout_index(&pane);

    if let FsPane::Rg {
        filtering,
        no_heading,
        ..
    } = pane
    {
        render.preview.scroll.index = Some("3".into());
        let r = &mut render.results;
        let s = &mut render.status;
        let mm = &cfg.styles.matchmaker;

        r.right_align_last = false;

        if !no_heading {
            r.horizontal_separator = mm.horizontal_separator;
            r.stacked_columns = true;
        }
        s.show = true;

        if !filtering {
            binds.insert(Event::QueryChange.into(), acs![FsAction::Reload]);
        }
    }

    let print_handle = AppendOnly::new();
    let tick_rate = render.tick_rate();

    // init MM
    let (mut mm, injector) = make_mm(
        render,
        tui,
        &cfg,
        print_handle.clone(),
        pane.stability_threshold(),
    );

    // init previewer
    let previewer_config = PreviewerConfig::default();
    let help_str = display_binds(&binds, Some(&previewer_config.help_colors));
    let previewer = make_previewer(&mut mm, previewer_config);

    let event_loop = EventLoop::with_binds(binds).with_tick_rate(tick_rate);
    let bind_tx = event_loop.bind_controller();
    let mut context = ActionContext::new();

    // configure mm
    let mut builder = PickOptions::new()
        .previewer(previewer)
        .event_loop(event_loop)
        .ext_handler(move |x, y| fsaction_handler(x, y, &mut context))
        .ext_aliaser(fsaction_aliaser)
        .paste_handler(paste_handler)
        .hidden_columns(vec![false, false, true])
        .matcher(MATCHER_CONFIG)
        .overlay_config(overlay)
        .overlay(StashOverlay::new(stash))
        .overlay(FilterOverlay::new(filters))
        .overlay(MenuOverlay::new(menu, prompt, cfg.actions));

    let render_tx = builder.render_tx();

    // start fs-watcher
    let (watcher, watcher_tx) = FsWatcher::new(cfg.notify, render_tx.clone());

    // set input
    render_tx
        .send(RenderCommand::Action(Action::SetPreview(Some(
            preview_layout_index,
        ))))
        .ok();
    match &pane {
        FsPane::Custom { input, .. }
        | FsPane::Nav { input, .. }
        | FsPane::Folders { input, .. }
        | FsPane::Files { input, .. } => {
            if !input.0.is_empty() {
                render_tx
                    .send(RenderCommand::Action(Action::SetQuery(input.0.clone())))
                    .ok();
            }
        }
        FsPane::Rg {
            patterns,
            input,
            filtering,
            ..
        } => {
            let f = *filtering;
            // set status line
            let base = if f {
                render_tx
                    .send(RenderCommand::Action(Action::SetQuery(input.0.clone())))
                    .ok();

                &cfg.global.panes.rg.fs_status_template
            } else {
                // enable auto-reload
                render_tx
                    .send(RenderCommand::Action(
                        FsAction::Filtering(Some(false)).into(),
                    ))
                    .ok();
                &cfg.global.panes.rg.rg_status_template
            };
            let mut t = StatusUI::parse_template_to_status_line(base);

            // perform other_query replacement
            let replacement = if f { &patterns.join(" / ") } else { &input.0 }; // todo: lowpri: styling
            for s in t.spans.iter_mut() {
                s.content = s.content.replace("{}", replacement).into();
            }

            render_tx
                .send(RenderCommand::Action(FsAction::SetStatus(Some(t)).into()))
                .ok();
        }
        _ => {}
    }

    // init history capabilities
    {
        let mut guard = DB_FILTER.lock().await;
        *guard = Some(DbFilter::new(&cfg.history));
    }

    // init global
    GLOBAL::init(cfg.global, render_tx, watcher_tx, db_pool, pane, bind_tx);
    clipboard::init(cfg.misc.clipboard_delay_ms);
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

    TASKS::shutdown(1, 3000).await;
    if APP::in_app_pane() {
        match ret.first().abort() {
            Ok(prog) => {
                // no contention, but clippy warning cannot be got rid of
                // let files = APP::TO_OPEN.lock().unwrap();
                let files = APP::TO_OPEN.lock().unwrap().clone();
                let conn = GLOBAL::db().get_conn(DbTable::apps).await?;

                let prog = Program::from_scanned_path(prog.path, prog.cmd);

                open_wrapped(conn, Some(prog), &files, true).await?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    } else {
        match ret {
            Ok(lines) if lines.is_empty() => Err(MatchError::NoMatch.into()),
            Ok(lines) => {
                let files: Vec<OsString> = lines
                    .iter()
                    .map(|p| OsString::from(p.path.inner()))
                    .collect();
                let conn = GLOBAL::db().get_conn(DbTable::apps).await?;
                let prog =
                    GLOBAL::with_env(|s| s.opener.as_ref().and_then(Program::from_os_string));
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
