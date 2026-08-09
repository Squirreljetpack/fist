use std::{borrow::Cow, ffi::OsString, sync::Arc};

use cba::{bog::BogOkExt, prints};
use fist_types::filters::SortOrder;
use matchmaker::{
    Either, MatchError, Matchmaker, PickOptions,
    config::{PreviewerConfig, RenderConfig, TerminalConfig},
    event::EventLoop,
    message::Event,
    nucleo::{Column, Text, Worker, injector::WorkerInjector},
    preview::AppendOnly,
};

use crate::{
    cli::CliOpts,
    clipboard,
    config::Config,
    db::{DbTable, Pool},
    display::{display_epoch, human_size},
    errors::CliError,
    run::{
        action::{fsaction_aliaser, fsaction_handler},
        ahandlers::{self, fs_post_reload_new, paste_handler},
        dhandlers::{MMExt, path_formatter, query_handler, sync_handler},
        item::PathItem,
        mm_config::{MATCHER_CONFIG, MMConfig},
        pane::FsPane,
        previewer::make_previewer,
        stash::STASH,
        state::{
            DB_FILTER, GLOBAL, STACK, TASKS,
            context::ActionContext,
            sort,
            ui::{global_ui_init, prompt_main_style},
        },
    },
    spawn::{Program, open_wrapped},
    ui::{
        confirm_overlay::ConfirmOverlay,
        filters_overlay::FilterOverlay,
        menu_overlay::MenuOverlay,
        stash_overlay::{ScratchOverlay, StashOverlay},
    },
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
    stability: u32,
) -> (FsMatchmaker, FsInjector) {
    let mut worker = Worker::new(
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
                // rg panes only: there order is None and metadata holds the packed loc
                if sort::get_sort().order.is_none() && item.value() != u64::MAX {
                    Text::from(item.loc().0.to_string())
                } else {
                    Text::from("")
                }
            })
            .without_filtering(),
        ],
        0,
        Arc::new(|_: &PathItem| Some(())),
        Arc::new(|_: &PathItem| ()),
    );
    worker.set_stability(stability);

    let injector = worker.injector();

    let mut mm = Matchmaker::new_on_cloneable(worker);

    mm.config_render(render);
    mm.config_tui(tui);

    mm.register_print_handler_(print_handle, template, separator);
    // attach previewer handling alt-h: help display, display file/fn
    mm.register_become_handler_();
    mm.register_execute_handler_();
    mm.register_reload_handler_();

    mm.register_event_handler(Event::Synced, sync_handler);
    mm.register_event_handler(Event::QueryChange, query_handler);

    (mm, injector)
}

/// Column 2 display: the sort value (mtime/atime/size) when hard-sorted with
/// an empty tail, else the tail text.
fn format_tail(item: &PathItem) -> Text<'static> {
    let order = sort::get_sort().order;
    // gated on an empty tail so the filter string (always the tail) can never
    // mismatch the display
    let show_value = matches!(
        order,
        Some(SortOrder::mtime | SortOrder::atime | SortOrder::size)
    ) && matches!(&item.tail, Ok([s, _]) if s.is_empty());
    if show_value {
        let v = item.value();
        if v == u64::MAX {
            // unset (e.g. size pre-fill) — render nothing
            return Text::from("");
        }
        return match order {
            Some(SortOrder::mtime | SortOrder::atime) => Text::from(display_epoch(v as i64)),
            Some(SortOrder::size) => Text::from(human_size(v, true)),
            _ => unreachable!(),
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
        enter_prompt,
        output,
        ..
    } = cli;

    // init configs
    let MMConfig {
        render,
        binds,
        stash,
        scratch,
        filters,
        prompt,
        menu,
        confirm,
        tui,
        overlay,
        help: _,
    } = mm_cfg;
    log::debug!("cfg: {cfg:?}");

    let print_handle = AppendOnly::new();
    let tick_rate = render.ui.tick_rate;

    // init MM
    let (mut mm, injector) = make_mm(
        render,
        tui,
        output.format.clone(),
        output.sep.clone().unwrap_or_else(|| "\n".into()),
        print_handle.clone(),
        pane.stability_threshold(),
    );

    let event_loop = EventLoop::with_binds(binds).with_tick_rate(tick_rate);
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
            fs_post_reload_new(state);
            if let Some(enter) = enter_prompt {
                ahandlers::enter_prompt(state, enter);
            };
            // kind of ugly but should reduce confusion
            if !state.picker_ui.results.cursor_disabled() {
                crate::run::state::FILTERS::with_vis_mut(|vis| {
                    if vis.dirs {
                        state
                            .picker_ui
                            .query
                            .set_prompt_line(ratatui::text::Line::styled(
                                "d: ",
                                prompt_main_style(),
                            ));
                    } else if vis.files {
                        state
                            .picker_ui
                            .query
                            .set_prompt_line(ratatui::text::Line::styled(
                                "f: ",
                                prompt_main_style(),
                            ));
                    }
                })
            }
        })
        .paste_handler(paste_handler)
        .hidden_columns(vec![2])
        .matcher(MATCHER_CONFIG)
        .overlay_config(overlay)
        .overlay(StashOverlay::new(stash.clone()))
        .overlay(ScratchOverlay::new(stash))
        .overlay(FilterOverlay::new(filters))
        .overlay(ConfirmOverlay::new(confirm))
        .overlay(MenuOverlay::new(menu, prompt, cfg.actions));

    let render_tx = builder.render_tx();

    // start fs-watcher
    let (watcher, watcher_tx) = FsWatcher::new(cfg.notify, render_tx.clone());

    // init history capabilities
    {
        let mut guard = DB_FILTER.lock().await;
        *guard = Some(cfg.history.clone());
    }
    // init global
    GLOBAL::init(
        cfg.global, cfg.stash, render_tx, watcher_tx, db_pool, pane, bind_tx,
    );
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

    TASKS::shutdown(500, 10, 3000).await;
    if STACK::in_app() {
        match ret {
            Ok(lines) if !lines.is_empty() => {
                let prog = &lines[0];
                let files = STASH::stashed_apps();
                let mut conn = GLOBAL::db().get_conn(DbTable::apps).await?;
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
                let conn = GLOBAL::db().get_conn(DbTable::apps).await?;
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
