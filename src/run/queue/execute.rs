use super::*;

use std::{fs::create_dir_all, sync::atomic::Ordering};

use cba::bs::symlink;
use fs_extra::{dir, file};

use crate::{
    cli::paths::actions_dir,
    run::{
        item::short_display,
        lua::{execute, load_script},
        state::{ToastStyle, MENU_ACTIONS, TOAST},
    },
};

impl QueueItem {
    /// Execute this item according to its kind:
    /// - `"copy"` / `"cut"` / `"symlink"` run the builtin transfer logic on
    ///   each source path, using the destination as-is (single-path items
    ///   are pre-resolved by the caller);
    /// - `"none"` is a no-op;
    /// - any other kind is a menu action key: the mapped command runs once
    ///   with the full path list, the destination, and the navigation
    ///   directory when it exists (`(paths, dst)` or
    ///   `(paths, dst, nav_cwd)`), and `set_progress` writes the item's
    ///   progress for the duration of the call. The progress is reset when
    ///   the call starts and marked complete afterwards so the display is
    ///   sensible even when the script never calls `set_progress`.
    ///
    /// A custom kind with no mapping fails the item with an error toast. The
    /// action history records one entry per executed item that completed
    /// successfully.
    pub fn execute(
        self,
        nav_cwd: Option<&AbsPath>,
    ) {
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
                // the kind is reserved; queue rows may still be enqueued
                // under it as an explicit no-op
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
                            progress.clone().store(fraction as u8, Ordering::Relaxed);
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
                            progress.clone().store(fraction as u8, Ordering::Relaxed);
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
                status.progress.store(0, Ordering::Relaxed);
                let result = load_script(&command, Some(actions_dir()))
                    .ok_or_else(|| anyhow::anyhow!("failed to load script"))
                    .and_then(|s| {
                        execute(
                            &s,
                            src,
                            &dst.to_string_lossy(),
                            nav_cwd,
                            Some(&status.progress),
                        )
                        .map_err(anyhow::Error::msg)
                    });
                // the run is done: report the progress as complete without
                // relying on the script calling `set_progress`
                status.progress.store(255, Ordering::Relaxed);

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
}
