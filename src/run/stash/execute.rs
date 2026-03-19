use super::*;

use std::{
    collections::{BTreeSet, HashMap},
    ffi::OsString,
    fs::create_dir_all,
    path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR},
    process::Command,
    sync::atomic::Ordering,
};

use cba::{
    bath::{PathExt, auto_dest_for_src},
    bs::symlink,
};
use fs_extra::{dir, file};

use crate::config::ExecuteStrategy;
use crate::{
    run::{
        item::short_display,
        state::{GLOBAL, TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

pub fn stash_formatter(
    src: &AbsPath,
    dst: &OsString,
    template: &str,
) -> String {
    // placeholder: create but don't implement
    template
        .replace("{1}", &src.to_string_lossy())
        .replace("{2}", &dst.to_string_lossy())
}

impl StashItem {
    pub fn execute(self) {
        log::debug!("Transferring: {self:?}");

        let Self {
            kind,
            src,
            status,
            dst,
        } = &self;

        status.state.store(StashItemState::Started);

        // determine strategy
        let (strategy, is_builtin) = if kind == "copy" {
            (ExecuteStrategy::Copy, true)
        } else if kind == "cut" {
            (ExecuteStrategy::Cut, true)
        } else if kind == "symlink" {
            (ExecuteStrategy::Symlink, true)
        } else {
            let mode = STASH::get_mode(kind);
            (mode.strategy, false)
        };

        if !is_builtin {
            match strategy {
                ExecuteStrategy::Symlink => {
                    match symlink(src, dst, true) {
                        Ok(()) => status.state.store(StashItemState::CompleteOk),
                        Err(_) => status.state.store(StashItemState::CompleteErr),
                    }
                    return;
                }
                ExecuteStrategy::Command(template) => {
                    let script = stash_formatter(src, dst, &template);
                    let shell = if cfg!(windows) { "cmd" } else { "sh" };
                    let arg = if cfg!(windows) { "/C" } else { "-c" };

                    let success = Command::new(shell)
                        .arg(arg)
                        .arg(&script)
                        .status()
                        .is_ok_and(|s| s.success());

                    if success {
                        status.state.store(StashItemState::CompleteOk);
                    } else {
                        status.state.store(StashItemState::CompleteErr);
                    }
                    return;
                }
                ExecuteStrategy::None => {
                    status.state.store(StashItemState::CompleteOk); // No-op
                    return;
                }
                _ => {} // Fallback to builtin for Copy/Cut if misconfigured
            }
        }

        // Built-in logic (Copy/Cut)
        let is_move = strategy == ExecuteStrategy::Cut;

        let StashItemStatus {
            state,
            progress,
            size,
        } = status;

        let result = if src.is_dir() {
            let mut options = dir::CopyOptions::new().copy_inside(true);
            options.overwrite = true;

            let progress_handler = move |p: dir::TransitProcess| {
                let fraction = if p.total_bytes > 0 {
                    size.store(p.total_bytes, Ordering::Relaxed);
                    p.copied_bytes * 255 / p.total_bytes
                } else {
                    0
                };
                progress.clone().store(fraction as u8, Ordering::Relaxed);
                fs_extra::dir::TransitProcessResult::ContinueOrAbort
            };

            if is_move {
                dir::move_dir_with_progress(src, dst, &options, progress_handler)
            } else {
                dir::copy_with_progress(src, dst, &options, progress_handler)
            }
        } else {
            let options = file::CopyOptions::new().overwrite(true);

            let progress_handler = move |p: file::TransitProcess| {
                let fraction = if p.total_bytes > 0 {
                    size.store(p.total_bytes, Ordering::Relaxed);
                    p.copied_bytes * 255 / p.total_bytes
                } else {
                    0
                };
                progress.clone().store(fraction as u8, Ordering::Relaxed);
            };

            if let Some(parent) = std::path::Path::new(dst).parent() {
                let _ = create_dir_all(parent);
            }

            if is_move {
                file::move_file_with_progress(src, dst, &options, progress_handler)
            } else {
                file::copy_with_progress(src, dst, &options, progress_handler)
            }
        };

        if let Err(e) = result {
            log::error!("Transfer error for {self:?}: {e}");
            state.store(StashItemState::CompleteErr);
            let display = short_display(src);
            TOAST::push(ToastStyle::Error, "Failed: ", [display]);
            TOAST::notice(ToastStyle::Error, e.to_string());
        } else {
            state.store(StashItemState::CompleteOk);
            let display = short_display(src);
            TOAST::push(ToastStyle::Success, "Complete: ", [display]);
            STASH_ACTION_HISTORY.lock().unwrap().push(self);
        }
    }
}

impl STASH {
    pub fn check_validity() {
        let state = STASH_STATE.lock().unwrap();
        for item in &state.shared {
            if item.status.state.is_pending() && !item.src.exists() {
                item.status.state.store(StashItemState::PendingErr)
            }
        }
    }

    pub fn execute_all_impl(
        base: AbsPath,
        include_completed: bool,
        indices: Option<&BTreeSet<usize>>,
    ) {
        let (queue, batch_groups): (Vec<StashItem>, HashMap<String, Vec<StashItem>>) = {
            let state = STASH_STATE.lock().unwrap();
            let mut q = vec![];
            let mut groups: HashMap<String, Vec<StashItem>> = HashMap::new();

            for (i, item) in state.shared.iter().enumerate() {
                if let Some(indices) = indices {
                    if !indices.contains(&i) {
                        continue;
                    }
                }
                let status = item.status.state.load();
                let should_transfer = match status {
                    StashItemState::Pending => true,
                    StashItemState::CompleteErr | StashItemState::CompleteOk
                        if include_completed =>
                    {
                        true
                    }
                    _ => false,
                };

                if should_transfer {
                    let mut item = item.clone();
                    let mut base_dest: OsString = item.dst.abs(&base).into();
                    if item.dst.to_string_lossy().ends_with(MAIN_SEPARATOR) || item.dst.is_empty() {
                        base_dest.push(MAIN_SEPARATOR_STR);
                    };
                    item.dst = GLOBAL::with_cfg(|c| {
                        auto_dest_for_src(&item.src, &base_dest, &c.fs.rename_policy)
                    })
                    .into();

                    let is_batch = STASH::get_mode(&item.kind).batch;

                    if is_batch {
                        groups.entry(item.kind.clone()).or_default().push(item);
                    } else {
                        q.push(item);
                    }
                }
            }
            (q, groups)
        };

        if !queue.is_empty() || !batch_groups.is_empty() {
            let total = queue.len() + batch_groups.values().map(|v| v.len()).sum::<usize>();
            TOAST::msg(format!("Starting {} items.", total), true);

            TASKS::spawn_blocking(move || {
                for (kind, items) in batch_groups {
                    for item in items {
                        item.execute();
                    }
                }
                for item in queue {
                    item.execute();
                }
            });
        } else {
            TOAST::msg("Stash is empty.", true);
        }
    }

    pub fn execute_all_scratch_impl(indices: Option<&BTreeSet<usize>>) {
        let state = STASH_STATE.lock().unwrap();
        let idx = state.current_scratch;
        let (kind, list) = &state.scratch[idx];
        let kind = kind.clone();
        let items: Vec<_> = list
            .as_slice()
            .into_iter()
            .enumerate()
            .filter(|(i, _)| indices.is_none_or(|ids: &BTreeSet<usize>| ids.contains(i)))
            .map(|(_, (src, dst))| {
                let mut item = StashItem::new(kind.clone(), src);
                item.dst = dst;
                item.dst = GLOBAL::with_cfg(|c| {
                    auto_dest_for_src(&item.src, &item.dst, &c.fs.rename_policy)
                })
                .into();
                item
            })
            .collect();

        if !items.is_empty() {
            TASKS::spawn_blocking(move || {
                for item in items {
                    item.execute();
                }
            });
        }
    }
}
