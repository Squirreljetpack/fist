mod status;
pub use status::*;

use std::{borrow::BorrowMut, cell::RefCell, path::MAIN_SEPARATOR, sync::atomic::Ordering};

use cli_boilerplate_automation::{
    bath::{PathExt, auto_dest_for_src},
    bs::symlink,
};
use fs_extra::{dir, file};

use crate::{
    abspath::AbsPath,
    cli::paths::home_dir,
    run::{
        item::short_display,
        state::{GLOBAL, TOAST},
    },
    utils::text::ToastStyle,
};

pub struct Stack {
    stack: Vec<StackItem>, // not indexmap because need const
}

#[derive(Copy, Clone, Debug, PartialEq, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum StackAction {
    Copy,
    Move,
    Symlink,
}

#[derive(Debug)]
pub struct StackItem {
    pub kind: StackAction,
    pub path: AbsPath,
    pub status: StackItemStatus,
    pub dest: String,
}

impl StackItem {
    pub fn cp(path: AbsPath) -> Self {
        Self {
            kind: StackAction::Copy,
            status: StackItemStatus::new(&path),
            path,
            dest: Default::default(),
        }
    }

    pub fn mv(path: AbsPath) -> Self {
        Self {
            kind: StackAction::Move,
            status: StackItemStatus::new(&path),
            path,
            dest: Default::default(),
        }
    }

    pub fn sym(path: AbsPath) -> Self {
        Self {
            kind: StackAction::Symlink,
            status: StackItemStatus::new(&path),
            path,
            dest: Default::default(),
        }
    }

    // spawn a task, pass the synced progress data to the task, on completion, change the status
    pub fn transfer(&self) {
        let src = self.path.clone();
        let status = self.status.clone();
        let action = self.kind;
        let dst = auto_dest_for_src(
            &self.path,
            &self.dest,
            &GLOBAL::with_cfg(|c| c.fs.rename_policy.clone()),
        );

        status.state.store(StackItemState::Started);

        if matches!(action, StackAction::Symlink) {
            match symlink(&src, &dst) {
                Ok(()) => {
                    status.state.store(StackItemState::CompleteOk);
                }
                Err(_) => {
                    status.state.store(StackItemState::CompleteErr);
                }
            }
        }

        let progress = status.progress.clone();
        let state = status.state.clone();
        let size = status.size.clone();

        log::debug!("Transferring: {self:?} -> dst: {dst:?}");

        tokio::task::spawn_blocking(move || {
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
                    StackAction::Copy => {
                        dir::copy_with_progress(&src, &dst, &options, progress_handler)
                    }
                    StackAction::Move => {
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
                    StackAction::Copy => {
                        file::copy_with_progress(&src, &dst, &options, progress_handler)
                    }
                    StackAction::Move => {
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
                state.store(StackItemState::CompleteErr);
                TOAST::push(ToastStyle::Error, "Failed: ", [short_display(&src)]);
            } else {
                state.store(StackItemState::CompleteOk);
                let display = short_display(&src); // dst.to_string_lossy().to_string().into()
                TOAST::push(ToastStyle::Success, "Complete: ", [display]);
            }
        });
    }

    pub fn display(&self) -> String {
        self.path.display_short(home_dir())
    }
}

impl Stack {
    pub const fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl std::ops::Deref for Stack {
    type Target = Vec<StackItem>;

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

impl PartialEq for StackItem {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.path == other.path
    }
}

impl Eq for StackItem {}

// -------- GLOBAL ---------
thread_local! {
    static SCRATCH_: RefCell<Stack> = const { RefCell::new(Stack::new()) };
}

pub struct STASH;

impl STASH {
    pub fn insert(items: impl IntoIterator<Item = StackItem>) {
        for item in items {
            SCRATCH_.with_borrow_mut(|s| insert_once(&mut s.stack, item, false));
        }
    }

    pub fn accept(index: usize) {
        SCRATCH_.with_borrow(|s| s.stack[index].transfer());
    }

    pub fn remove(index: usize) {
        SCRATCH_.with_borrow_mut(|s| s.stack.remove(index));
    }

    pub fn with<R>(f: impl FnOnce(&Stack) -> R) -> R {
        SCRATCH_.with(|cell| f(&cell.borrow()))
    }

    pub fn with_mut<R>(f: impl FnOnce(&mut Stack) -> R) -> R {
        SCRATCH_.with_borrow_mut(|cell| f(cell.borrow_mut()))
    }

    // call on overlay enable
    pub fn check_validity() {
        SCRATCH_.with_borrow(|s| {
            for item in &s.stack {
                if !item.path.exists() {
                    item.status.state.store(StackItemState::PendingErr)
                }
            }
        });
    }
    pub fn transfer_all(
        base: &AbsPath,
        include_completed: bool,
    ) {
        let mut toast_vec = Vec::new();

        STASH::with_mut(|s| {
            for item in s.stack.iter_mut() {
                let item_state = item.status.state.load();
                if item_state == StackItemState::Pending
                    || (include_completed
                        && matches!(
                            item_state,
                            StackItemState::CompleteErr | StackItemState::CompleteOk
                        ))
                {
                    let mut resolved = item.dest.abs(base).to_string_lossy().to_string();

                    if item.dest.ends_with(MAIN_SEPARATOR) || item.dest.is_empty() {
                        resolved.push(MAIN_SEPARATOR);
                    };
                    item.dest = resolved;

                    item.transfer();
                    // > 10 mb
                    if item.status.size.load(Ordering::Acquire) > 1024 * 1024 * 10 {
                        toast_vec.push(short_display(&item.path));
                    }
                }
            }
        });

        if !toast_vec.is_empty() {
            TOAST::push(ToastStyle::Normal, "Started: ", toast_vec);
        }
    }

    pub fn clear_invalid_and_completed() {
        SCRATCH_.with_borrow_mut(|s| {
            s.stack.retain(|item| !item.status.state.is_complete());
        });
    }
}
