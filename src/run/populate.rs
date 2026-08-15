use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt::Display,
    io::{self, Read},
    path::Path,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use ansi_to_tui::IntoText;
use cba::{
    bait::ResultExt,
    bath::PathExt,
    bo::{map_chunks, map_reader_lines, read_to_chunks},
    bog::BogOkExt,
    bring::StrExt,
    broc::{CommandExt, display_sh_prog_and_args},
    unwrap,
};
use matchmaker::{SSS, message::RenderCommand, nucleo::injector::Injector};
use tokio::task::spawn_blocking;

use crate::{
    abspath::AbsPath,
    db::DbTable,
    find::{apps::collect_apps, fd::build_fd_args, walker::list_dir},
    run::{
        FsAction,
        item::PathItem,
        lua::call_transform,
        start::FsInjector,
        state::{APP, GLOBAL, STACK, TASKS, sort},
    },
};
use crate::{
    config::GlobalConfig,
    find::rg::{build_rg_args, is_inverted},
    run::{
        FsPane,
        populate_rg::{
            BufItem, MultilineRgParser, flush_rg_buffer, process_rg_line,
        },
        state::{STORE, ShouldNotAbortOnEmpty, TOAST},
    },
};
use TASKS::TaskId;
use fist_types::filters::SortOrder;

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
                cmd,
                stored,
                cwd,
                complete,
                tail_sep: delim,
                input_sep: record_sep,
                transform,
                ..
            } => {
                complete.store(false, Ordering::SeqCst);
                if let Some(stored) = stored {
                    stored.map_to_vec(|item| {
                        sort::store_sort_value(item, sort::get_sort().order);
                        injector.push(item.clone())
                    });
                    if complete.load(Ordering::SeqCst) {
                        return None;
                    }
                }

                let delim = *delim;
                let cwd = cwd.clone();
                let stored = stored.clone();
                let complete = complete.clone();
                let transform = transform.clone();

                // Some = spawn and read its stdout; None = read stdin
                let stdout: Box<dyn Read + Send + Sync> = match cmd {
                    Some((prog, args)) => {
                        log::info!("spawning: {}", display_sh_prog_and_args(prog, args));

                        let (child, stdout) = Command::new(prog)
                            .args(args)
                            .current_dir(&cwd)
                            .spawn_piped()
                            ._ebog()?;
                        TASKS::register_child(TaskId::Populate, child);
                        Box::new(stdout)
                    }
                    None => Box::new(io::stdin()),
                };

                map_reader(
                    stdout,
                    *record_sep,
                    move |line| {
                        let [first, raw_tail] = line.split_delim(delim);
                        let (path, display, tail) = if let Some(f) = transform.as_ref() {
                            let p = AbsPath::new_unchecked(first.abs(&cwd));
                            let (path, display, tail) = call_transform(f, &p, raw_tail)?;
                            // A missing path omits the entry from the listing.
                            let Some(path) = path else {
                                return anyhow::Ok(());
                            };
                            (
                                path,
                                display.unwrap_or_default(),
                                tail.unwrap_or_else(|| raw_tail.to_string()),
                            )
                        } else {
                            (first.to_string(), String::new(), raw_tail.to_string())
                        };
                        let mut item = PathItem::new_unchecked(path.abs(&cwd));
                        item.tail = Ok([tail, display]);
                        sort::store_sort_value(&item, sort::get_sort().order);
                        if let Some(stored) = &stored {
                            stored.push(item.clone());
                        };
                        injector.push(item)?;
                        anyhow::Ok(())
                    },
                    move |count| {
                        if count == Some(0) {
                            GLOBAL::send_mm(RenderCommand::NoMatch);
                        }
                        complete.store(true, Ordering::SeqCst);
                    },
                )
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
                transform,
                ..
            } => {
                let vis = *vis;
                let cwd = cwd.clone();
                let threshold = cfg.panes.find.max_refresh_items_threshold;
                let time_threshold = cfg.panes.find.max_refresh_execution_time_threshold;
                let start_time = std::time::Instant::now();
                let (prog, args) = ("fd", build_fd_args(vis, types, paths, fd_args, &cfg.fd));

                log::info!("spawning: {}", display_sh_prog_and_args(prog, &args));

                let (child, stdout) = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;
                TASKS::register_child(TaskId::Populate, child);

                // not sure about this, but this kinda works to ensure only first run of fs will abort
                let abort_empty = STORE::get::<ShouldNotAbortOnEmpty>().is_none();
                if abort_empty {
                    STORE::set(ShouldNotAbortOnEmpty {});
                }

                let _complete = complete.clone();
                let _cwd = cwd.clone();
                let transform = transform.clone();
                map_reader(
                    stdout,
                    Some('\0'),
                    move |line| {
                        let item = if let Some(f) = transform.as_ref() {
                            let p = AbsPath::new_unchecked(line.abs(&cwd));
                            let (path, display, tail) = call_transform(f, &p, "")?;
                            // A missing path omits the entry from the listing.
                            let Some(path) = path else {
                                return anyhow::Ok(());
                            };
                            let mut item = PathItem::new(path, &cwd);
                            item.tail = Ok([tail.unwrap_or_default(), display.unwrap_or_default()]);
                            item
                        } else {
                            PathItem::new(line, &cwd)
                        };
                        let push = vis.post_fd_filter(&item.path);

                        if push {
                            sort::store_sort_value(&item, sort::get_sort().order);
                            injector.push(item)?;
                        }
                        anyhow::Ok(())
                    },
                    move |count| {
                        if count == Some(0) {
                            if abort_empty {
                                GLOBAL::send_mm(RenderCommand::NoMatch);
                            } else if toast_on_empty {
                                TOAST::toast_empty();
                            }
                        }

                        // lowpri: theoretically this should be immune to triggering after pane changes
                        // as push would Err but we should have a test
                        if let Some(c) = count
                            && ({ c == 0 || c < threshold } && {
                                time_threshold.is_zero() || start_time.elapsed() < time_threshold
                            })
                        {
                            GLOBAL::send_watcher(crate::watcher::WatcherMessage::Switch(
                                _cwd.inner(),
                                notify::RecursiveMode::Recursive,
                            ));
                        }

                        _complete.store(true, Ordering::SeqCst);
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
                one_line,
                patterns,
                paths,
                rg,
                complete,
                fixed_strings,
                //
                filtering: _,
                input: _,
                is_initial,
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
                        *one_line,
                        *fixed_strings,
                        patterns,
                        paths,
                        rg,
                        &cfg.rg,
                    ),
                );
                let toast_on_empty = toast_on_empty && is_initial.take();
                let no_column = is_inverted(&args);

                log::info!("spawning: {}", display_sh_prog_and_args(prog, &args));

                let (child, stdout) = Command::new(prog)
                    .args(args)
                    .current_dir(&cwd)
                    .spawn_piped()
                    ._ebog()?;
                TASKS::register_child(TaskId::Populate, child);

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

                let _complete = complete.clone();
                if *one_line {
                    map_reader_rg(
                        stdout,
                        *context,
                        &cwd,
                        no_column,
                        injector,
                        toast_on_empty,
                        complete.clone(),
                    )
                } else {
                    let mut parser = MultilineRgParser::new();
                    let cwd_ = cwd.clone();
                    let vis_ = vis;
                    let injector_ = injector.clone();

                    map_reader(
                        stdout,
                        None,
                        move |line| {
                            parser.process_line(
                                line,
                                &cwd_,
                                no_column,
                                vis_,
                                |item| {
                                    let _ = injector_.push(item);
                                },
                            )
                        },
                        move |count| {
                            if count == Some(0) {
                                if toast_on_empty {
                                    TOAST::toast_empty();
                                }
                            }
                            _complete.store(true, Ordering::SeqCst);
                        },
                    )
                }
            }
            Self::Files { sort, .. } => {
                let sort = *sort;
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::files).await.elog()?;
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;
                    if entries.is_empty() && toast_on_empty {
                        TOAST::toast_empty();
                    }

                    for e in entries {
                        let item = PathItem::new_unchecked(e.path.into());
                        injector.push(item)?;
                    }

                    Ok(())
                })
            }
            Self::Folders { sort, .. } => {
                let sort = *sort;
                let cwd = STACK::_cwd();
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::dirs).await.elog()?;
                    let entries = GLOBAL::get_db_entries(&mut conn, sort).await?;
                    if entries.is_empty() && toast_on_empty {
                        TOAST::toast_empty();
                    }

                    let mut entries = entries.into_iter();

                    // skip the first cwd item
                    if matches!(sort, SortOrder::atime) {
                        if let Some(e) = entries.next()
                            && e.path != cwd
                        {
                            let item = PathItem::new_unchecked(e.path.into());
                            injector.push(item)?
                        }
                    }

                    for e in entries {
                        let item = PathItem::new_unchecked(e.path.into());
                        injector.push(item)?
                    }

                    Ok(())
                })
            }
            Self::Stash { stash_name, .. } => {
                let stash_name = stash_name.clone();
                let filter_missing = cfg.panes.stash.filter_missing;
                let prune = cfg.panes.stash.prune;
                let pool = GLOBAL::db();

                tokio::spawn(async move {
                    let mut conn = pool.get_conn(DbTable::stashes).await.elog()?;
                    let entries = conn.get_stash_entries(&stash_name).await.elog()?;
                    if entries.is_empty() && toast_on_empty {
                        TOAST::toast_empty();
                    }

                    // with prune, nonexistent entries are collected and
                    // removed from the db after the loop
                    let mut to_prune: Vec<AbsPath> = Vec::new();
                    for e in entries {
                        if !e.stash.exists() {
                            if prune {
                                to_prune.push(e.stash.clone());
                                continue;
                            }
                            if filter_missing {
                                continue;
                            }
                        }
                        let mut item = PathItem::new_unchecked(e.stash.into());
                        item.tail = Ok([e.tail, String::new()]);
                        sort::store_sort_value(&item, sort::get_sort().order);
                        injector.push(item)?;
                    }

                    if !to_prune.is_empty() {
                        conn.remove_stash_entries(&stash_name, &to_prune)
                            .await
                            .elog()?;
                        log::debug!(
                            "Pruned {} missing entr{} from stash ({stash_name})",
                            to_prune.len(),
                            if to_prune.len() == 1 { "y" } else { "ies" },
                        );
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
                    let mut empty = toast_on_empty;

                    for path in list_dir(&cwd, vis, depth) {
                        empty = false;
                        let item = PathItem::new_unchecked(path);
                        sort::store_sort_value(&item, sort::get_sort().order);
                        injector.push(item)?
                    }

                    if empty {
                        TOAST::toast_empty();
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
    delimiter: Option<char>,
    mut f: impl FnMut(String) -> Result<(), E> + SSS,
    complete: impl FnOnce(Option<usize>) + SSS,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    spawn_blocking(move || {
        let count = if let Some(c) = delimiter {
            map_chunks::<E>(read_to_chunks(reader, c), &mut f, true)
        } else {
            map_reader_lines::<E>(reader, &mut f, true)
        }
        ._elog();

        let _ = f(String::new());

        complete(count);
        log::info!("Command completed");
        anyhow::Ok(())
    })
}

// todo: lowpri: rg --null adds null byte after filepaths
pub fn map_reader_rg(
    reader: impl Read + matchmaker::SSS + 'static,
    context: [usize; 2],
    cwd: &Path,
    no_column: bool,
    injector: FsInjector,
    toast_on_empty: bool,
    complete: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<anyhow::Result<()>> {
    let cwd = cwd.to_path_buf();
    spawn_blocking(move || {
        let buffer = RefCell::new(VecDeque::<BufItem>::with_capacity(
            context[0] + context[1] + 1,
        ));

        let mut path_buffer = String::new();

        let count = map_reader_lines::<anyhow::Error>(
            reader,
            |line| {
                let failed_to_parse = |e| {
                    log::error!("ParseError: {e}: {line}");
                    Ok(())
                };

                if !line.contains('\0') {
                    if line == "--" {
                        let mut buf = buffer.borrow_mut();
                        flush_rg_buffer(context, &cwd, &mut buf, |item| {
                            let _ = injector.push(item);
                        });
                        return Ok(());
                    }
                    path_buffer.push_str(&line);
                    path_buffer.push('\n');
                    return Ok(());
                }

                let prefix = if path_buffer.is_empty() {
                    None
                } else {
                    Some(std::mem::take(&mut path_buffer))
                };

                let mut text = unwrap!(line.as_bytes().into_text(); |e| failed_to_parse(e));

                if text.lines.is_empty() {
                    return failed_to_parse("empty".into());
                }

                let mut buf = buffer.borrow_mut();
                process_rg_line(
                    text.lines.remove(0),
                    prefix.as_deref(),
                    context,
                    &cwd,
                    no_column,
                    &mut buf,
                    |item| {
                        let _ = injector.push(item);
                    },
                )?;

                Ok(())
            },
            true,
        )
        ._elog();

        if count == Some(0) {
            if toast_on_empty {
                TOAST::toast_empty();
            }
        } else {
            let mut buf = buffer.borrow_mut();
            flush_rg_buffer(context, &cwd, &mut buf, |item| {
                let _ = injector.push(item);
            });
        }

        complete.store(true, Ordering::SeqCst);

        log::info!("Command completed");
        anyhow::Ok(())
    })
}
