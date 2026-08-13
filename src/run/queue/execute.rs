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
use mlua::MultiValue;

use crate::{
    run::{
        item::short_display,
        lua::{compile_lua, load_script},
        state::{GLOBAL, TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

impl QueueItem {
    /// Execute this item according to its kind:
    /// - `"copy"` / `"cut"` / `"symlink"` run the builtin transfer logic;
    /// - any other kind is a script reference (`@file` syntax supported) that
    ///   is fed `(item, dest)`; a script that fails to load/compile is a no-op.
    ///
    /// Every source path is transferred; single-path items carry a resolved
    /// destination, multi-path items (menu Stash/Batch) resolve each path's
    /// destination against the nav cwd.
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

        for path in src {
            let path_dst: OsString = if src.len() == 1 || !dst.is_empty() {
                dst.clone()
            } else if let Some(base) = STACK::nav_cwd() {
                let mut d: OsString = dst.abs(base).into();
                if dst.is_empty() || dst.to_string_lossy().ends_with(MAIN_SEPARATOR) {
                    d.push(MAIN_SEPARATOR_STR);
                };
                GLOBAL::with_cfg(|c| auto_dest_for_src(path, &d, &c.fs.rename_policy)).into()
            } else {
                dst.clone()
            };

            match kind.as_str() {
                "symlink" => {
                    match symlink(path, &path_dst, true) {
                        Ok(()) => status.state.store(QueueItemState::CompleteOk),
                        Err(_) => status.state.store(QueueItemState::CompleteErr),
                    }
                    continue;
                }
                "none" => {
                    status.state.store(QueueItemState::CompleteOk); // No-op
                    continue;
                }
                "copy" | "cut" => {}
                // any other kind is a script reference
                script => {
                    let result = load_script(script)
                        .ok_or_else(|| anyhow::anyhow!("failed to load script"))
                        .and_then(|s| compile_lua(&s).map_err(anyhow::Error::msg))
                        .and_then(|f| {
                            let item = path.to_string_lossy();
                            let dest = path_dst.to_string_lossy();
                            f.call::<MultiValue>((item.as_ref(), dest.as_ref()))
                                .map_err(anyhow::Error::from)
                        });

                    match result {
                        Ok(_) => status.state.store(QueueItemState::CompleteOk),
                        Err(e) => {
                            log::error!("Queue script error for {self:?}: {e}");
                            status.state.store(QueueItemState::CompleteErr);
                        }
                    }
                    continue;
                }
            }

            // Built-in logic (Copy/Cut)
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
                    dir::move_dir_with_progress(path, &path_dst, &options, progress_handler)
                } else {
                    dir::copy_with_progress(path, &path_dst, &options, progress_handler)
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

                if let Some(parent) = std::path::Path::new(&path_dst).parent() {
                    let _ = create_dir_all(parent);
                }

                if is_move {
                    file::move_file_with_progress(path, &path_dst, &options, progress_handler)
                } else {
                    file::copy_with_progress(path, &path_dst, &options, progress_handler)
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
                QUEUE_ACTION_HISTORY.lock().unwrap().push(self.clone());
            }
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
