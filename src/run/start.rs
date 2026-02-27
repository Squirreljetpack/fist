use std::{ffi::OsString, sync::Arc};

use cli_boilerplate_automation::{
    bog::BogOkExt,
    bring::{StrExt, split::join_with_single_quotes},
    prints, unwrap,
};
use matchmaker::{
    MatchError, MatchResultExt, Matchmaker, PickOptions, RenderFn, Selector,
    binds::display_binds,
    config::{PreviewerConfig, RenderConfig, TerminalConfig},
    event::EventLoop,
    message::Event,
    nucleo::{
        Column, Indexed, Render, Worker,
        injector::{IndexedInjector, WorkerInjector},
    },
    preview::AppendOnly,
};
use ratatui::text::Text;

use crate::{
    cli::env::EnvOpts,
    clipboard,
    config::Config,
    db::{DbTable, Pool, zoxide::DbFilter},
    errors::CliError,
    run::{
        action::{fsaction_aliaser, fsaction_handler},
        ahandlers::{fs_post_reload_new, paste_handler},
        dhandlers::{MMExt, query_handler, sync_handler},
        item::PathItem,
        mm_config::{MATCHER_CONFIG, MMConfig},
        pane::FsPane,
        previewer::make_previewer,
        stash::STASH,
        state::{DB_FILTER, GLOBAL, STACK, TASKS, context::ActionContext, ui::global_ui_init},
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
    cfg: &Config,
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

    mm.register_print_handler_(
        print_handle,
        cfg.misc.output_template.clone(),
        cfg.misc.output_separator.clone(),
    );
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
    mut cfg: Config,
    mm_cfg: MMConfig,
    db_pool: Pool,
) -> Result<(), CliError> {
    // init configs
    let MMConfig {
        mut render,
        binds,
        stash,
        filters,
        prompt,
        menu,
        tui,
        overlay,
    } = mm_cfg;
    log::debug!("cfg: {cfg:?}");

    let print_handle = AppendOnly::new();
    let tick_rate = render.tick_rate();

    EnvOpts::with_env(|e| {
        if let Some(v) = e.output_template.clone() {
            cfg.misc.output_template = Some(v);
        }
        if let Some(v) = e.output_separator.clone() {
            cfg.misc.output_separator = v;
        }
        Some(())
    });

    match &pane {
        FsPane::Search {
            patterns,
            filtering,
            ..
        } => {
            if !filtering {
                render.input.initial = join_with_single_quotes(patterns);
            }
        }
        FsPane::Find { .. } => {}
        _ => {}
    }

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
    let mut context = ActionContext::new(print_handle.clone());

    // configure mm
    let mut builder = PickOptions::new()
        .previewer(previewer)
        .event_loop(event_loop)
        .ext_handler(move |x, y| fsaction_handler(x, y, &mut context))
        .ext_aliaser(fsaction_aliaser)
        .initializer(fs_post_reload_new)
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
    if STACK::in_app() {
        match ret.first().abort() {
            Ok(prog) => {
                let files = STASH::stashed_apps();
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
                set_envs(&lines);

                let files: Vec<OsString> = lines
                    .iter()
                    .map(|p| OsString::from(p.path.inner()))
                    .collect();
                let conn = GLOBAL::db().get_conn(DbTable::apps).await?;
                let prog =
                    EnvOpts::with_env(|s| s.opener.as_ref().and_then(Program::from_os_string));
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
            let s = unwrap!(lines[0].cmd.as_ref());
            let [line, rest] = s.split_delim(':');
            let line = unwrap!(line.parse::<usize>().ok());
            let col = rest.split_delim(':')[0].parse::<usize>().ok();
            Some((line, col))
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
