use std::{
    fmt::Display,
    io::{self, Read},
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ansi_to_tui::IntoText;
use anyhow::bail;
use cli_boilerplate_automation::{
    bait::ResultExt,
    bo::{MapReaderError, map_reader_lines},
    bog::BogOkExt,
    bring::StrExt,
    broc::{CommandExt, display_sh_prog_and_args},
    bs::sort_by_mtime,
    unwrap,
};
use matchmaker::{SSS, message::RenderCommand, nucleo::injector::Injector, preview::AppendOnly};
use ratatui::text::Text;
use tokio::task::spawn_blocking;

use crate::{
    abspath::AbsPath,
    cli::env::EnvOpts,
    config::GlobalConfig,
    find::rg::build_rg_args,
    run::{FsPane, state::TOAST},
    utils::text::{extract_rg_line_no_path, parse_rg_line, scrub_text_styles, text_to_lines},
};
use crate::{
    db::{DbSortOrder, DbTable},
    find::{apps::collect_apps, fd::build_fd_args, size::sort_by_size, walker::list_dir},
    run::{
        FsAction,
        item::PathItem,
        start::FsInjector,
        state::{APP, GLOBAL, STACK},
    },
};
use fist_types::filters::{SortOrder, Visibility};

// todo: when do we need be able to restart after STOP
// todo: lowpri: this is like 1.2 slower than pure fd? Accept input is a bit sluggish? Cache to reduce disk reads?
// Doesn't block, uses tokio::spawn
impl FsPane {
    pub fn populate(
        &self,
        injector: FsInjector,
        cfg: &GlobalConfig,
        _callback: impl FnOnce() + 'static + Send + Sync,
    ) -> Option<tokio::task::JoinHandle<anyhow::Result<()>>> {
        log::debug!("Populating: {self:?}");
        let toast_on_empty = GLOBAL::with_cfg(|c| c.interface.toast_on_empty);

        let ret = match self {
            Self::Custom {
                cmd: (prog, args),
                stored,
                cwd,
                vis,
                sort: _,
                complete,
                ..
            } => {
                complete.store(false, Ordering::SeqCst);
                if let Some(stored) = stored {
                    stored.map_to_vec(|item| injector.push(item.clone()));
                    if complete.load(Ordering::SeqCst) {
                        return None;
                    }
                }

                let delim = EnvOpts::with_env(|c| c.delim);
                let display_script = EnvOpts::with_env(|c| c.display.clone());
                let vis = *vis;
                let cwd = cwd.clone();
                let stored = stored.clone();
                let complete = complete.clone();
                let _complete = complete.clone();

                log::info!("spawning: {}", display_sh_prog_and_args(prog, args));

                let stdout = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;

                let sem = Arc::new(tokio::sync::Semaphore::new(GLOBAL::with_cfg(|c| {
                    c.panes.settings.display_script_simultaneous_count
                })));

                match display_script {
                    None => map_reader(
                        stdout,
                        move |line| {
                            let item = PathItem::new_from_split(line.split_delim(delim), &cwd);
                            if vis.filter(&item.path) {
                                if let Some(stored) = &stored {
                                    stored.push(item.clone());
                                };
                                injector.push(item)?;
                            }
                            anyhow::Ok(())
                        },
                        complete.clone(),
                        || GLOBAL::send_mm(RenderCommand::QuitEmpty),
                    ),
                    Some(Ok(script)) => {
                        // Script runs per item asynchronously
                        map_reader(
                            stdout,
                            move |line| {
                                if complete.load(Ordering::SeqCst) {
                                    bail!("Canceled");
                                }
                                let injector = injector.clone();
                                let sem = sem.clone();
                                let cwd = cwd.clone();
                                let script = script.clone();
                                let stored = stored.clone();
                                let _complete = complete.clone();

                                tokio::spawn(async move {
                                    let _permit = sem.acquire_owned().await.unwrap();
                                    if let Ok(out) =
                                        crate::spawn::utils::tokio_command_from_script(&script)
                                            .args(line.split_delim(delim))
                                            .output()
                                            .await
                                    {
                                        let mut item =
                                            PathItem::new_from_split(line.split_delim(delim), &cwd);
                                        if out.status.success() {
                                            if let Ok(rendered) =
                                                ansi_to_tui::IntoText::into_text(&out.stdout)
                                            {
                                                item.override_rendered(rendered);
                                            }
                                        }
                                        if let Some(stored) = &stored {
                                            stored.push(item.clone());
                                        };
                                        if injector.push(item).is_err() {
                                            _complete.store(true, Ordering::SeqCst);
                                        }
                                    }
                                });
                                Ok(())
                            },
                            _complete.clone(),
                            || GLOBAL::send_mm(RenderCommand::QuitEmpty),
                        )
                    }
                    Some(Err(script)) => map_reader_batch(
                        stdout,
                        complete.clone(),
                        || GLOBAL::send_mm(RenderCommand::QuitEmpty),
                        script.clone(),
                        cwd,
                        stored.clone(),
                        vis,
                        delim,
                        injector,
                    ),
                }
            }

            // exactly the same as custom
            Self::Stream {
                stored,
                cwd,
                vis,
                sort: _,
                complete,
                ..
            } => {
                complete.store(false, Ordering::SeqCst);
                if let Some(stored) = stored {
                    stored.map_to_vec(|item| injector.push(item.clone()));
                    if complete.load(Ordering::SeqCst) {
                        return None;
                    }
                }

                let delim = EnvOpts::with_env(|c| c.delim);
                let display_script = EnvOpts::with_env(|c| c.display.clone());
                let cwd = cwd.clone();
                let vis = *vis;
                let stored = stored.clone();
                let complete = complete.clone();
                let _complete = complete.clone();

                let stdout = io::stdin();
                let sem = Arc::new(tokio::sync::Semaphore::new(GLOBAL::with_cfg(|c| {
                    c.panes.settings.display_script_simultaneous_count
                })));

                match display_script {
                    None => map_reader(
                        stdout,
                        move |line| {
                            let item = PathItem::new_from_split(line.split_delim(delim), &cwd);
                            if vis.filter(&item.path) {
                                if let Some(stored) = &stored {
                                    stored.push(item.clone());
                                };
                                injector.push(item)?;
                            }
                            anyhow::Ok(())
                        },
                        complete.clone(),
                        || GLOBAL::send_mm(RenderCommand::QuitEmpty),
                    ),
                    Some(Ok(script)) => {
                        // Script runs per item asynchronously
                        map_reader(
                            stdout,
                            move |line| {
                                if complete.load(Ordering::SeqCst) {
                                    bail!("Canceled");
                                }
                                let injector = injector.clone();
                                let sem = sem.clone();
                                let cwd = cwd.clone();
                                let script = script.clone();
                                let stored = stored.clone();
                                let _complete = complete.clone();

                                tokio::spawn(async move {
                                    let _permit = sem.acquire_owned().await.unwrap();
                                    if let Ok(out) =
                                        crate::spawn::utils::tokio_command_from_script(&script)
                                            .args(line.split_delim(delim))
                                            .output()
                                            .await
                                    {
                                        let mut item =
                                            PathItem::new_from_split(line.split_delim(delim), &cwd);
                                        if out.status.success() {
                                            if let Ok(rendered) =
                                                ansi_to_tui::IntoText::into_text(&out.stdout)
                                            {
                                                item.override_rendered(rendered);
                                            }
                                        }
                                        if let Some(stored) = &stored {
                                            stored.push(item.clone());
                                        };
                                        if injector.push(item).is_err() {
                                            _complete.store(true, Ordering::SeqCst);
                                        }
                                    }
                                });
                                Ok(())
                            },
                            _complete.clone(),
                            || GLOBAL::send_mm(RenderCommand::QuitEmpty),
                        )
                    }
                    Some(Err(script)) => map_reader_batch(
                        stdout,
                        complete.clone(),
                        || GLOBAL::send_mm(RenderCommand::QuitEmpty),
                        script.clone(),
                        cwd,
                        stored.clone(),
                        vis,
                        delim,
                        injector,
                    ),
                }
            }

            Self::Find {
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

                let abort_empty = STACK::len() == 1;

                map_reader(
                    stdout,
                    move |line| {
                        let item = PathItem::new(line, &cwd);
                        let push = vis.post_fd_filter(&item.path);

                        if push { injector.push(item) } else { Ok(()) }
                    },
                    complete.clone(),
                    move || {
                        if abort_empty {
                            GLOBAL::send_mm(RenderCommand::QuitEmpty);
                        } else if toast_on_empty {
                            TOAST::toast_empty();
                        }
                    },
                )
            }
            Self::Search {
                cwd,
                //
                vis,
                sort,
                //
                context,
                case,
                no_heading,
                patterns,
                paths,
                rg,
                complete,
                fixed_strings,
                //
                filtering,
                input,
            } => {
                let vis = *vis;
                let cwd = cwd.clone();
                let (prog, args) = (
                    "rg",
                    build_rg_args(
                        vis,
                        *sort,
                        *context,
                        *case,
                        *no_heading,
                        *fixed_strings,
                        patterns,
                        paths,
                        rg,
                        &cfg.rg,
                    ),
                );

                log::info!("spawning: {}", display_sh_prog_and_args(prog, &args));

                let stdout = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;

                // Example output of rg 'command' --column --case-sensitive -C 2
                // src/components/settings/ClamshellMicrophoneSelector.tsx
                // 1-import React, { useState, useEffect } from "react";
                // 2-import { useTranslation } from "react-i18next";
                // 3:10:import { commands } from "@/bindings";
                // 4-import { Dropdown } from "../ui/Dropdown";
                // 5-import { SettingContainer } from "../ui/SettingContainer";
                // --
                // 30-      const checkIsLaptop = async () => {
                // 31-        try {
                // 32:32:          const result = await commands.isLaptop();
                // 33-          if (result.status === "ok") {
                // 34-            setIsLaptop(result.data);
                //
                // src/components/settings/PostProcessingSettingsApi/usePostProcessProviderState.ts
                // 1-import { useCallback, useMemo, useState } from "react";
                // 2-import { useSettings } from "../../../hooks/useSettings";
                // 3:10:import { commands, type PostProcessProvider } from "@/bindings";
                // 4-import type { ModelOption } from "./types";
                // 5-import type { DropdownOption } from "../../ui/Dropdown";
                // --
                // 82-      // Check Apple Intelligence availability before selecting
                // 83-      if (providerId === APPLE_PROVIDER_ID) {
                // 84:33:        const available = await commands.checkAppleIntelligenceAvailable();
                // 85-        if (!available) {
                // 86-          setAppleIntelligenceUnavailable(true);

                // with --no-heading:
                // --
                // node_modules/@babel/parser/CHANGELOG.md-487-![image](https://cloud.githubusercontent.com/assets/5233399/19420267/388f556e-93ad-11e6-813e-7c5c396be322.png)
                // node_modules/@babel/parser/CHANGELOG.md-488-
                // --

                // So : => present, - => context
                // empty line => next line is path
                // -- => context break

                // haven't yet tested multiline
                // possible extensions: seperate items for each context block, parsing blocks for line numbers

                if *no_heading {
                    map_reader(
                        stdout,
                        move |line| {
                            let failed_to_parse = |e| {
                                log::error!("ParseError: {e}: {line}");
                                anyhow::Ok(())
                            };
                            let mut text =
                                unwrap!(line.as_bytes().into_text(); |e| failed_to_parse(e));
                            if text.lines.is_empty() {
                                return failed_to_parse("empty".into());
                            }
                            let (path, data, mut text) = unwrap!(parse_rg_line(text.lines.remove(0), ':'); failed_to_parse("failed to split".into()));
                            // skip empty lines
                            if text.lines.iter().all(|l| l.spans.is_empty()) {
                                return Ok(());
                            }
                            scrub_text_styles(&mut text);

                            let mut item = PathItem::new(path, &cwd);
                            item.cmd = Some(data);
                            item.tail = text;

                            injector.push(item).cast()
                        },
                        complete.clone(),
                        move || {
                            if toast_on_empty {
                                TOAST::toast_empty();
                            }
                        },
                    )
                } else {
                    let mut current_path = String::new();
                    // let mut current_line_data = Text::default();
                    let mut current_context = vec![];
                    let mut current_places = String::new();

                    map_reader(
                        stdout,
                        move |line| {
                            if current_path.is_empty() {
                                // rg emits ansi resets if we enable color
                                current_path = line
                                    .as_bytes()
                                    .into_text()
                                    .ok()
                                    .and_then(|x| text_to_lines(&x).first().cloned())
                                    .unwrap_or_default();
                                anyhow::Ok(())
                            } else if line.is_empty() {
                                if current_path.is_empty() {
                                    current_context.clear();
                                    return Ok(());
                                }
                                let mut item =
                                    PathItem::new(std::mem::take(&mut current_path), &cwd);
                                let mut text = Text::from(std::mem::take(&mut current_context));
                                scrub_text_styles(&mut text);
                                for line in &text.lines {
                                    extract_rg_line_no_path(line, &mut current_places);
                                }

                                item.tail = text;
                                item.cmd = Some(std::mem::take(&mut current_places));

                                let push = vis.post_fd_filter(&item.path);
                                if push {
                                    injector.push(item).cast()
                                } else {
                                    Ok(())
                                }
                            } else {
                                current_context.extend(
                                    line.as_bytes()
                                        .into_text()
                                        .unwrap_or(Text::from_iter([line])),
                                );
                                anyhow::Ok(())
                            }
                        },
                        complete.clone(),
                        move || {
                            if toast_on_empty {
                                TOAST::toast_empty();
                            }
                        },
                    )
                }
            }
            Self::Files { sort, .. } => {
                let sort = *sort;
                let cwd = STACK::cwd().unwrap_or_default();
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::files).await.elog()?;
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;
                    if entries.is_empty() && toast_on_empty {
                        TOAST::toast_empty();
                    }

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
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;
                    if entries.is_empty() && toast_on_empty {
                        TOAST::toast_empty();
                    }

                    let mut entries = entries.into_iter();

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
            Self::Apps { sort, .. } => {
                let sort = *sort;
                let pool = GLOBAL::db();
                let pool_clone = pool.clone();

                let ret = tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::apps).await.elog()?;
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;

                    if toast_on_empty && entries.is_empty() {
                        TOAST::toast_empty();
                    }

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

                    let mut files: Vec<PathBuf> = iter.collect();

                    match sort {
                        //                         files.sort_by(|a, b| {
                        //     a.file_name()
                        //         .to_string_lossy()
                        //         .to_lowercase()
                        //         .cmp(&b.file_name().to_string_lossy().to_lowercase())
                        // });
                        // Case sensitive
                        SortOrder::name => files.sort_by(|a, b| a.file_name().cmp(&b.file_name())),
                        SortOrder::mtime => sort_by_mtime(&mut files),
                        SortOrder::size => sort_by_size(&mut files),
                        SortOrder::none => {}
                    }

                    if files.is_empty() && toast_on_empty {
                        TOAST::toast_empty();
                    }

                    for path in files.into_iter() {
                        let item = PathItem::new_unchecked(path, &cwd);
                        injector.push(item)?
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
    f: impl FnMut(String) -> Result<(), E> + SSS,
    complete: Arc<AtomicBool>,
    on_empty: impl FnOnce() + SSS,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    spawn_blocking(move || {
        let count = map_reader_lines::<true, E>(reader, f)._elog();
        match count {
            Some(0) => on_empty(),
            _ => {}
        }
        complete.store(true, Ordering::SeqCst);
        log::info!("Command completed");
        anyhow::Ok(())
    })
}

pub fn map_reader_batch(
    reader: impl Read + matchmaker::SSS,
    complete: Arc<AtomicBool>,
    on_empty: impl FnOnce() + SSS,
    script: String,
    cwd: AbsPath,
    stored: Option<AppendOnly<PathItem>>,
    vis: Visibility,
    delim: Option<char>,
    injector: FsInjector,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    let batch_collect_size = GLOBAL::with_cfg(|c| c.panes.settings.display_script_batch_size);
    let mut batch_collect = Vec::with_capacity(batch_collect_size);
    let _complete = complete.clone();

    spawn_blocking(move || {
        let count = map_reader_lines::<true, _>(reader, |line| {
            if complete.load(Ordering::SeqCst) {
                bail!("Canceled");
            }
            let [p1, p2] = line.split_delim(delim);
            batch_collect.push([p1.to_string(), p2.to_string()]);

            if batch_collect.len() >= batch_collect_size {
                let batch = std::mem::take(&mut batch_collect);
                let batch_count = batch.len();
                let injector = injector.clone();
                let cwd = cwd.clone();
                let script = script.clone();
                let stored = stored.clone();
                let _complete = complete.clone();

                // the maybe better would be to use tokio::spawn + spawn_piped_tokio, but then we need an async read version of map_reader_lines
                tokio::task::spawn_blocking(move || {
                    if let Some(stdout) = Command::from_script(&script)
                    .args(batch.iter().flatten())
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()
                    {
                        let mut batch_iter = batch.into_iter();

                        match map_reader_lines::<true, ()>(stdout, move |line| {
                            let [p1, p2] = unwrap!(batch_iter.next(), ());
                            {
                                let mut item =
                                PathItem::new_from_split([&p1, &p2], &cwd);

                                if let Ok(rendered) =
                                ansi_to_tui::IntoText::into_text(&line)
                                {
                                    item.override_rendered(rendered);
                                };

                                if let Some(stored) = &stored {
                                    stored.push(item.clone());
                                };
                                injector
                                .push(item)
                                .map_err(|_| _complete.store(true, Ordering::SeqCst))
                            }
                        }) {
                            Ok(n) if n < batch_count => {
                                log::warn!(
                                    "{} items missing after processing display-batch", batch_count - n
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
                        _complete.store(true, Ordering::SeqCst);
                    }
                });
            }

            Ok(())
        })._elog();
        if !batch_collect.is_empty() && !complete.load(Ordering::SeqCst) {
            let batch = batch_collect;
            let batch_count = batch.len();
            let _complete = complete.clone();

            if let Some(stdout) = Command::from_script(&script)
                .args(batch.iter().flatten())
                .current_dir(&cwd)
                .spawn_piped()
                ._ebog()
            {
                let mut batch_iter = batch.into_iter();

                match map_reader_lines::<true, ()>(stdout, move |line| {
                    let [p1, p2] = unwrap!(batch_iter.next(), ());
                    {
                        let mut item = PathItem::new_from_split([&p1, &p2], &cwd);

                        if let Ok(rendered) = ansi_to_tui::IntoText::into_text(&line) {
                            item.override_rendered(rendered);
                        };

                        if let Some(stored) = &stored {
                            stored.push(item.clone());
                        };
                        injector
                            .push(item)
                            .map_err(|_| _complete.store(true, Ordering::SeqCst))
                    }
                }) {
                    Ok(n) if n < batch_count => {
                        log::warn!(
                            "{} items missing after processing display-batch",
                            batch_count - n
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
                .extend(
                    batch
                        .into_iter()
                        .map(|[p1, p2]| PathItem::new_from_split([&p1, &p2], &cwd)),
                )
                .is_err()
            {
                complete.store(true, Ordering::SeqCst);
            }
        }

        match count {
            Some(0) => on_empty(),
            _ => {}
        }
        complete.store(true, Ordering::SeqCst);
        log::info!("Command completed");
        anyhow::Ok(())
    })
}
