mod status;
pub use status::*;
use tokio::task::spawn_blocking;

use std::{
    borrow::BorrowMut,
    cell::RefCell,
    ffi::OsString,
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
    cli::paths::__home,
    run::{
        item::short_display,
        state::{GLOBAL, TOAST},
    },
    utils::text::ToastStyle,
};

pub struct Stack {
    stack: Vec<StashItem>, // not indexmap because need const
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum StashAction {
    Copy,
    Move,
    Symlink,
}

#[derive(Debug, Clone)]
pub struct StashItem {
    pub kind: StashAction,
    pub path: AbsPath,
    pub status: StashItemStatus,
    pub dest: OsString,
}

impl StashItem {
    pub fn cp(path: AbsPath) -> Self {
        Self {
            kind: StashAction::Copy,
            status: StashItemStatus::new(&path),
            path,
            dest: Default::default(),
        }
    }

    pub fn mv(path: AbsPath) -> Self {
        Self {
            kind: StashAction::Move,
            status: StashItemStatus::new(&path),
            path,
            dest: Default::default(),
        }
    }

    pub fn sym(path: AbsPath) -> Self {
        Self {
            kind: StashAction::Symlink,
            status: StashItemStatus::new(&path),
            path,
            dest: Default::default(),
        }
    }

    // blocking
    // on completion, change the status
    pub fn transfer(self) {
        log::debug!("Transferring: {self:?}");

        let src = self.path;
        let dst = self.dest;
        let status = self.status;
        let action = self.kind;

        status.state.store(StashItemState::Started);

        if matches!(action, StashAction::Symlink) {
            match symlink(&src, &dst) {
                Ok(()) => {
                    status.state.store(StashItemState::CompleteOk);
                }
                Err(_) => {
                    status.state.store(StashItemState::CompleteErr);
                }
            }
        }

        let StashItemStatus {
            state,
            progress,
            size,
        } = status;

        let result = if src.is_dir() {
            // options: todo
            let mut options = dir::CopyOptions::new();
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

            match action {
                StashAction::Copy => {
                    dir::copy_with_progress(&src, &dst, &options, progress_handler)
                }
                StashAction::Move => {
                    dir::move_dir_with_progress(&src, &dst, &options, progress_handler)
                }
                _ => {
                    unreachable!()
                }
            }
        } else {
            // options: todo
            let mut options = file::CopyOptions::new();
            options.overwrite = true;

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

            match action {
                StashAction::Copy => {
                    file::copy_with_progress(&src, &dst, &options, progress_handler)
                }
                StashAction::Move => {
                    file::move_file_with_progress(&src, &dst, &options, progress_handler)
                }
                _ => {
                    unreachable!()
                }
            }
        };

        if let Err(e) = result {
            log::error!(
                "Transfer error for {} -> {}: {e}",
                src,
                dst.to_string_lossy()
            );
            state.store(StashItemState::CompleteErr);
            let display = short_display(&src);
            TOAST::push(ToastStyle::Error, "Failed: ", [display]);
        } else {
            state.store(StashItemState::CompleteOk);
            let display = short_display(&src);
            TOAST::push(ToastStyle::Success, "Complete: ", [display]);
        }
    }

    pub fn display(&self) -> String {
        self.path.display_short(__home())
    }
}

impl Stack {
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl std::ops::Deref for Stack {
    type Target = Vec<StashItem>;

    fn deref(&self) -> &Self::Target {
        &self.stack
    }
}

// helpers

// pub fn toggle_insert<T: PartialEq>(
//     list: &mut Vec<T>,
//     item: T,
// ) {
//     if let Some(i) = list.iter().position(|x| *x == item) {
//         list.remove(i);
//     } else {
//         list.push(item);
//     }
// }

pub fn insert_once<T: PartialEq>(
    list: &mut Vec<T>,
    item: T,
    stable: bool,
) {
    if stable {
        if !list.contains(&item) {
            list.push(item);
        }
    } else {
        if let Some(i) = list.iter().position(|x| *x == item) {
            list.remove(i);
        }
        list.push(item);
    }
}

impl PartialEq for StashItem {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.path == other.path
    }
}

impl Eq for StashItem {}

// -------- GLOBAL ---------
thread_local! {
    static STASH_: RefCell<Stack> = const { RefCell::new(Stack::new()) };
}

pub struct STASH;

impl STASH {
    pub fn insert(items: impl IntoIterator<Item = StashItem>) {
        for item in items {
            STASH_.with_borrow_mut(|s| insert_once(&mut s.stack, item, false));
        }
    }

    pub fn accept(index: usize) {
        STASH_.with_borrow(|s| {
            let mut item = s.stack[index].clone();
            item.dest = GLOBAL::with_cfg(|c| {
                auto_dest_for_src(&item.path, &item.dest, &c.fs.rename_policy)
            })
            .into();
            spawn_blocking(|| item.transfer());
        });
    }

    pub fn remove(index: usize) {
        STASH_.with_borrow_mut(|s| s.stack.remove(index));
    }

    pub fn with<R>(f: impl FnOnce(&Stack) -> R) -> R {
        STASH_.with(|cell| f(&cell.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut Stack) -> R) -> R {
        STASH_.with_borrow_mut(|cell| f(cell.borrow_mut()))
    }

    // call on overlay enable
    pub fn check_validity() {
        STASH_.with_borrow(|s| {
            for item in &s.stack {
                if item.status.state.is_pending() && !item.path.exists() {
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
        let queue: Vec<_> = STASH::with_mut(|s| {
            s.iter()
                .cloned()
                .filter_map(|mut item| {
                    // normalize dest
                    let mut base_dest: OsString = item.dest.abs(&base).into();
                    // empty dest -> paste into current
                    if item.dest.to_string_lossy().ends_with(MAIN_SEPARATOR) || item.dest.is_empty()
                    {
                        base_dest.push(MAIN_SEPARATOR_STR);
                    };
                    item.dest = GLOBAL::with_cfg(|c| {
                        auto_dest_for_src(&item.path, &base_dest, &c.fs.rename_policy)
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
                .collect()
        });

        if !queue.is_empty() {
            TOAST::push_notice(
                ToastStyle::Normal,
                format!("Starting {} items.", queue.len()),
            );
        };
        spawn_blocking(move || {
            for item in queue {
                item.transfer();
            }
        });
    }

    pub fn clear_invalid_and_completed() {
        STASH_.with_borrow_mut(|s| {
            s.stack.retain(|item| {
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
        STASH_.with_borrow_mut(|s| {
            s.stack.retain(|item| !item.status.state.is_complete());
        });
    }
}
