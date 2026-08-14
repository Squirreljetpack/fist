use super::*;

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs::create_dir_all,
    path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR},
    sync::atomic::Ordering,
};

use cba::{
    bath::{PathExt, auto_dest_for_src},
    bs::symlink,
};
use fs_extra::{dir, file};

use crate::{
    run::{
        item::short_display,
        lua::{call_with_paths, compile_lua, load_script},
        state::{MENU_ACTIONS, TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

impl QueueItem {
    /// Execute this item according to its kind:
    /// - `"copy"` / `"cut"` / `"symlink"` run the builtin transfer logic on
    ///   each source path, using the destination as-is (single-path items
    ///   are pre-resolved by the caller);
    /// - `"none"` is a no-op;
    /// - any other kind is a menu action key: the mapped command runs once
    ///   with the full path list and the destination (`(paths, dst)`), and
    ///   `set_progress` writes the item's progress for the duration of the
    ///   call.
    ///
    /// A custom kind with no mapping fails the item with an error toast. The
    /// action history records one entry per executed item that completed
    /// successfully.
    pub fn execute(self) {
        log::debug!("Transferring: {self:?}");

        let Self {
            kind,
            src,
            status,
            dst,
        } = &self;

        status.state.store(QueueItemState::Started);

        let is_move = kind == "cut";

        let mut any_success = false;

        match kind.as_str() {
            "symlink" => {
                for path in src {
                    match symlink(path, dst, true) {
                        Ok(()) => {
                            status.state.store(QueueItemState::CompleteOk);
                            any_success = true;
                        }
                        Err(_) => status.state.store(QueueItemState::CompleteErr),
                    }
                }
            }
            "none" => {
                // defensive: the kind is reserved, so nothing creates a
                // "none" item today; keep the historical no-op behavior
                status.state.store(QueueItemState::CompleteOk);
                any_success = true;
            }
            "copy" | "cut" => {
                for path in src {
                    let QueueItemStatus {
                        state,
                        progress,
                        size,
                    } = status;

                    let result = if path.is_dir() {
                        let mut options = dir::CopyOptions::new().copy_inside(true);
                        options.overwrite = true;

                        let progress_handler = move |p: dir::TransitProcess| {
                            let fraction = if p.total_bytes > 0 {
                                size.store(p.total_bytes, Ordering::Relaxed);
                                p.copied_bytes * 255 / p.total_bytes
                            } else {
                                0
                            };
                            progress
                                .clone()
                                .store(fraction as u8, Ordering::Relaxed);
                            fs_extra::dir::TransitProcessResult::ContinueOrAbort
                        };

                        if is_move {
                            dir::move_dir_with_progress(path, dst, &options, progress_handler)
                        } else {
                            dir::copy_with_progress(path, dst, &options, progress_handler)
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
                            progress
                                .clone()
                                .store(fraction as u8, Ordering::Relaxed);
                        };

                        if let Some(parent) = std::path::Path::new(dst).parent() {
                            let _ = create_dir_all(parent);
                        }

                        if is_move {
                            file::move_file_with_progress(path, dst, &options, progress_handler)
                        } else {
                            file::copy_with_progress(path, dst, &options, progress_handler)
                        }
                    };

                    if let Err(e) = result {
                        log::error!("Transfer error for {self:?}: {e}");
                        state.store(QueueItemState::CompleteErr);
                        let display = short_display(path);
                        TOAST::push(ToastStyle::Error, "Failed: ", [display]);
                        TOAST::notice(ToastStyle::Error, e.to_string());
                    } else {
                        state.store(QueueItemState::CompleteOk);
                        let display = short_display(path);
                        TOAST::push(ToastStyle::Success, "Complete: ", [display]);
                        any_success = true;
                    }
                }
            }
            // any other kind is a menu action key
            script => {
                let command = match MENU_ACTIONS.get().and_then(|m| m.get(script)) {
                    Some(action) => action.command.clone(),
                    None => {
                        log::error!("No menu action for queue kind {script:?}: {self:?}");
                        status.state.store(QueueItemState::CompleteErr);
                        TOAST::notice(
                            ToastStyle::Error,
                            format!("No menu action for kind {script}"),
                        );
                        return;
                    }
                };
                let result = load_script(&command)
                    .ok_or_else(|| anyhow::anyhow!("failed to load script"))
                    .and_then(|s| compile_lua(&s).map_err(anyhow::Error::msg))
                    .and_then(|f| {
                        call_with_paths(&f, src, &dst.to_string_lossy(), Some(&status.progress))
                            .map_err(anyhow::Error::from)
                    });

                match result {
                    Ok(_) => {
                        status.state.store(QueueItemState::CompleteOk);
                        any_success = true;
                    }
                    Err(e) => {
                        log::error!("Queue script error for {self:?}: {e}");
                        status.state.store(QueueItemState::CompleteErr);
                    }
                }
            }
        }

        if any_success {
            QUEUE_ACTION_HISTORY.lock().unwrap().push(self.clone());
        }
    }
}

impl QUEUE {
    pub fn check_validity() {
        let state = QUEUE_STATE.lock().unwrap();
        for item in &state.shared {
            if item.status.state.is_pending() && !item.src.iter().all(|p| p.exists()) {
                item.status.state.store(QueueItemState::PendingErr)
            }
        }
    }

    /// Queue every pending (or completed, if asked) shared item for
    /// execution, resolving destinations against `base`.
    pub fn execute_all_impl(
        base: AbsPath,
        include_completed: bool,
        indices: Option<&BTreeSet<usize>>,
    ) {
        let queue: Vec<QueueItem> = {
            let state = QUEUE_STATE.lock().unwrap();
            let mut q = vec![];

            for (i, item) in state.shared.iter().enumerate() {
                if let Some(indices) = indices
                    && !indices.contains(&i)
                {
                    continue;
                }
                let status = item.status.state.load();
                let should_transfer = match status {
                    QueueItemState::Pending => true,
                    QueueItemState::CompleteErr | QueueItemState::CompleteOk
                        if include_completed =>
                    {
                        true
                    }
                    _ => false,
                };

                if should_transfer {
                    let mut item = item.clone();
                    // multi-path items (menu Stash/Batch) resolve each path's
                    // destination inside `QueueItem::execute`; single-path
                    // items resolve it here against the base
                    if item.src.len() == 1 {
                        let mut base_dest: OsString = item.dst.abs(&base).into();
                        if item.dst.to_string_lossy().ends_with(MAIN_SEPARATOR)
                            || item.dst.is_empty()
                        {
                            base_dest.push(MAIN_SEPARATOR_STR);
                        };
                        item.dst = GLOBAL::with_cfg(|c| {
                            auto_dest_for_src(&item.src[0], &base_dest, &c.fs.rename_policy)
                        })
                        .into();
                    }
                    q.push(item);
                }
            }
            q
        };

        if !queue.is_empty() {
            TOAST::msg(format!("Starting {} items.", queue.len()), true);

            TASKS::spawn_blocking(move || {
                for item in queue {
                    item.execute();
                }
            });
        } else {
            TOAST::msg("Queue is empty.", true);
        }
    }
}
