use std::{ffi::OsString, sync::Arc};

use cli_boilerplate_automation::{bait::ResultExt, bog::BogOkExt, prints};
use matchmaker::{
    MatchError, MatchResultExt, Matchmaker, PickOptions, RenderFn, Selector,
    binds::display_binds,
    config::{PreviewerConfig, RenderConfig, TerminalConfig},
    make_previewer,
    message::Event,
    nucleo::{
        Column, Indexed, Render, Worker,
        injector::{IndexedInjector, WorkerInjector},
    },
    preview::AppendOnly,
    render::Effect,
};

use crate::{
    clipboard,
    config::Config,
    db::{DbTable, Pool, zoxide::DbFilter},
    errors::CliError,
    run::{
        action::{fsaction_aliaser, fsaction_handler, paste_handler},
        dhandlers::{MMExt, mm_formatter, sync_handler},
        item::PathItem,
        mm_config::{MATCHER_CONFIG, MMConfig},
        pane::FsPane,
        state::{APP, DB_FILTER, GLOBAL, STACK},
    },
    spawn::{Program, open_wrapped},
    ui::{
        filters_overlay::FilterOverlay, global::global_ui_init, menu_overlay::MenuOverlay,
        stash_overlay::StashOverlay,
    },
    watcher::FsWatcher,
};

pub type FsInjector = IndexedInjector<PathItem, WorkerInjector<Indexed<PathItem>>>;

fn exist_validator(s: &PathItem) -> bool {
    s.path.exists()
}

pub type FormatterFn = Arc<RenderFn<Indexed<PathItem>>>;

// todo: prompt needs to show initial fs pattern if given
fn make_mm(
    render: RenderConfig,
    tui: TerminalConfig,
    cfg: &Config,
    print_handle: AppendOnly<String>,
) -> (
    Matchmaker<Indexed<PathItem>, PathItem>,
    FsInjector,
    FormatterFn,
) {
    let worker = Worker::new(
        [
            Column::new("_", |item: &Indexed<PathItem>| item.inner.as_text()),
            Column::new("", |item: &Indexed<PathItem>| item.inner.tail.clone()),
        ],
        0,
    );
    let injector = IndexedInjector::new_globally_indexed(worker.injector());

    let selector = Selector::new_with_validator(Indexed::identifier, exist_validator);
    // todo: we really just want an alternate behavior for accept multi
    // if cfg.global.interface.no_multi {
    //     selector = selector.disabled()
    // }

    #[allow(clippy::type_complexity)]
    let formatter: Arc<Box<dyn Fn(&Indexed<PathItem>, &str) -> String + Send + Sync>> =
        Arc::new(Box::new(mm_formatter));

    let mut mm = Matchmaker::new(worker, selector);

    mm.config_render(render);
    mm.config_tui(tui);

    mm.register_print_handler_(print_handle);
    // attach previewer handling alt-h: help display, display file/fn
    mm.register_become_handler_();
    mm.register_execute_handler_();
    mm.register_reload_handler_();
    mm.register_event_handler(Event::Synced, sync_handler);

    (mm, injector, formatter)
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
        binds,
        scratch,
        filters,
        prompt,
        menu,
        tui,
    } = mm_cfg;
    log::debug!("cfg: {cfg:?}");

    if let Some(x) = pane.preview_show(&cfg.global.panes) {
        render.preview.show = x
    }
    if let Some(x) = pane.prompt(&cfg.global.panes) {
        render.input.prompt = x
    }
    let print_handle = AppendOnly::new();
    // init MM
    let (mut mm, injector, formatter) = make_mm(render, tui, &cfg, print_handle.clone());

    // init previewer
    let previewer_config = PreviewerConfig::default();
    let help_str = display_binds(&binds, Some(&previewer_config.help_colors));
    let previewer = make_previewer(&mut mm, previewer_config, formatter, help_str);

    // configure mm
    let mut builder = PickOptions::with_binds(binds)
        .previewer(previewer)
        .ext_handler(fsaction_handler)
        .ext_aliaser(fsaction_aliaser)
        .paste_handler(paste_handler)
        .matcher(MATCHER_CONFIG)
        .overlay(StashOverlay::new(scratch))
        .overlay(FilterOverlay::new(filters))
        .overlay(MenuOverlay::new(menu, prompt));

    let render_tx = builder.render_tx();

    // start fs-watcher
    let (watcher, watcher_tx) = FsWatcher::new(cfg.notify, render_tx.clone());

    // set input
    match &pane {
        FsPane::Custom { input, .. }
        | FsPane::Nav { input, .. }
        | FsPane::Folders { input, .. }
        | FsPane::Files { input, .. } => {
            let il = input.0.len() as u16;
            render_tx
                .send(matchmaker::message::RenderCommand::Effect(Effect::Input((
                    input.0.clone(),
                    il,
                ))))
                .elog()
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
    GLOBAL::init(cfg.global, render_tx, watcher_tx, db_pool, pane);
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

    if APP::in_app_pane() {
        match ret.first().abort() {
            Ok(prog) => {
                // no contention, but clippy warning cannot be got rid of
                // let files = APP::TO_OPEN.lock().unwrap();
                let files = APP::TO_OPEN.lock().unwrap().clone();
                let conn = GLOBAL::db().get_conn(DbTable::apps).await?;

                let prog = Program::from_scanned_path(prog.path, prog.cmd);

                open_wrapped(conn, Some(prog), &files).await?;
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
                // the default is the same behavior as fs :open, which also called by fs :tool lessfilter open
                open_wrapped(
                    conn,
                    GLOBAL::with_env(|s| s.opener.as_ref().and_then(Program::from_os_string)),
                    &files,
                )
                .await?;
                Ok(())
            }
            Err(MatchError::Abort(i)) => std::process::exit(i),
            Err(e) => Err(e.into()),
        }
    }
}
