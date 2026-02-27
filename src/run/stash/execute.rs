use super::*;
use ratatui::text::Line;

use std::{
    ffi::OsString,
    fs::create_dir_all,
    path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR},
    sync::atomic::Ordering,
};

use cli_boilerplate_automation::{
    bath::{PathExt, auto_dest_for_src},
    bs::symlink,
};
use fs_extra::{dir, file};

use crate::{
    abspath::AbsPath,
    run::{
        item::short_display,
        state::{GLOBAL, TASKS, TOAST},
    },
    utils::text::ToastStyle,
};

impl StashItem {
    // blocking
    // on completion, change the status
    pub fn transfer(
        self,
        custom_action_state: CustomStashActionActionState,
    ) {
        log::debug!("Transferring: {self:?}");

        let Self {
            kind,
            src,
            status,
            dst,
        } = &self;

        status.state.store(StashItemState::Started);

        if matches!(kind, StashAction::Custom) {
            match custom_action_state {
                CustomStashActionActionState::Symln => match symlink(src, dst) {
                    Ok(()) => {
                        status.state.store(StashItemState::CompleteOk);
                    }
                    Err(_) => {
                        status.state.store(StashItemState::CompleteErr);
                    }
                },
                _ => {}
            }
            return;
        }

        let StashItemStatus {
            state,
            progress,
            size,
        } = status;

        let result = if src.is_dir() {
            // options: todo
            let mut options = dir::CopyOptions::new().copy_inside(true);
            options.overwrite = true;

            let progress_handler = move |p: dir::TransitProcess| {
                // store progress
                let fraction = if p.total_bytes > 0 {
                    size.store(p.total_bytes, Ordering::Relaxed);
                    p.copied_bytes * 255 / p.total_bytes
                } else {
                    0
                };
                progress.clone().store(fraction as u8, Ordering::Relaxed);

                fs_extra::dir::TransitProcessResult::ContinueOrAbort
            };

            match kind {
                StashAction::Copy => dir::copy_with_progress(src, dst, &options, progress_handler),
                StashAction::Move => {
                    dir::move_dir_with_progress(src, dst, &options, progress_handler)
                }
                _ => {
                    unreachable!()
                }
            }
        } else {
            // options: todo
            let options = file::CopyOptions::new().overwrite(true);

            let progress_handler = move |p: file::TransitProcess| {
                // store progress
                let fraction = if p.total_bytes > 0 {
                    size.store(p.total_bytes, Ordering::Relaxed);
                    p.copied_bytes * 255 / p.total_bytes
                } else {
                    0
                };
                progress.clone().store(fraction as u8, Ordering::Relaxed);
            };

            if true && let Some(parent) = std::path::Path::new(dst).parent() {
                let _ = create_dir_all(parent); // error will be caught by copy
            }

            match kind {
                StashAction::Copy => file::copy_with_progress(src, dst, &options, progress_handler),
                StashAction::Move => {
                    file::move_file_with_progress(src, dst, &options, progress_handler)
                }
                _ => {
                    unreachable!()
                }
            }
        };

        if let Err(e) = result {
            log::error!("Transfer error for {self:?}: {e}");
            state.store(StashItemState::CompleteErr);
            let display = short_display(src);
            TOAST::push(ToastStyle::Error, "Failed: ", [display]);
            TOAST::push_notice(ToastStyle::Error, e.to_string());
        } else {
            state.store(StashItemState::CompleteOk);
            let display = short_display(src);
            TOAST::push(ToastStyle::Success, "Complete: ", [display]);
            STASH_ACTION_HISTORY.lock().unwrap().push(self);
        }
    }
}

impl STASH {
    // call on overlay enable
    pub fn check_validity() {
        MAIN_STASH.with_borrow(|s| {
            for item in &s.0.stack {
                if item.status.state.is_pending() && !item.src.exists() {
                    item.status.state.store(StashItemState::PendingErr)
                }
            }
            log::debug!("stash validated.");
        });
    }

    /// spawns a queue to transfer all items
    pub fn transfer_all(
        base: AbsPath,
        include_completed: bool, // not sure if we ever want this
    ) {
        let (queue, custom_action): (Vec<_>, _) = STASH::with_mut(|s| {
            let (s, c) = (s.0, *s.1);

            if matches!(c, CustomStashActionActionState::App) {
                return (vec![], c);
            }

            let queue = s
                .iter()
                .cloned()
                .filter_map(|mut item| {
                    if item.is_custom() && matches!(c, CustomStashActionActionState::App) {
                        return None;
                    }
                    // normalize dest
                    let mut base_dest: OsString = item.dst.abs(&base).into();
                    // empty dest -> paste into current
                    if item.dst.to_string_lossy().ends_with(MAIN_SEPARATOR) || item.dst.is_empty() {
                        base_dest.push(MAIN_SEPARATOR_STR);
                    };
                    item.dst = GLOBAL::with_cfg(|c| {
                        auto_dest_for_src(&item.src, &base_dest, &c.fs.rename_policy)
                    })
                    .into();
                    let ret = match item.status.state.load() {
                        StashItemState::Pending => true,
                        StashItemState::CompleteErr | StashItemState::CompleteOk
                            if include_completed =>
                        {
                            true
                        }
                        _ => false,
                    };
                    ret.then_some(item)
                })
                .collect();

            (queue, c)
        });

        if !queue.is_empty() {
            TOAST::push_msg(
                Line::styled(
                    format!("Starting {} items.", queue.len()),
                    ToastStyle::Normal,
                ),
                true,
            );

            TASKS::spawn_blocking(move || {
                for item in queue {
                    item.transfer(custom_action);
                }
            });
        };
    }

    pub fn clear_invalid_and_completed() {
        MAIN_STASH.with_borrow_mut(|s| {
            s.0.stack.retain(|item| {
                !matches!(
                    item.status.state.load(),
                    StashItemState::CompleteOk
                        | StashItemState::CompleteErr
                        | StashItemState::PendingErr
                )
            });
        });
    }

    pub fn clear_completed() {
        MAIN_STASH.with_borrow_mut(|s| {
            s.0.stack.retain(|item| !item.status.state.is_complete());
        });
    }

    pub fn clear(clear_custom_only: Option<bool>) {
        STASH::retain(|item| {
            let started = matches!(item.status.state.load(), StashItemState::Started);

            started
                || match clear_custom_only {
                    None => false,
                    Some(true) => !item.is_custom(),
                    Some(false) => item.is_custom(),
                }
        });
    }

    pub fn retain<F>(mut f: F)
    where
        F: FnMut(&StashItem) -> bool,
    {
        MAIN_STASH.with(|cell| {
            let mut s = cell.borrow_mut();
            s.0.stack.retain(|item| f(item));
        });
    }
}
