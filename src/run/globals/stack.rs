#![allow(clippy::upper_case_acronyms)]

use std::{cell::RefCell, env::current_dir};

use log::{self};
use matchmaker::SSS;

use crate::{
    abspath::AbsPath,
    run::{
        FsInjector, FsPane,
        state::{FILTERS, GLOBAL, TEMP},
    },
    ui::global::global_ui_mut,
    watcher::WatcherMessage,
};

thread_local! {
    static STACK: RefCell<STACK> = const { RefCell::new(STACK::new()) }
}

pub struct STACK {
    stack: Vec<FsPane>,
    index: usize,
    count: usize,
}
impl STACK {
    const fn new() -> Self {
        Self {
            stack: Vec::new(),
            index: 0,
            count: 1,
        }
    }

    pub fn init(pane: FsPane) {
        STACK.with(|s| {
            *s.borrow_mut() = Self {
                stack: vec![pane],
                index: 0,
                count: 1,
            }
        });
    }

    pub fn push(pane: FsPane) {
        STACK.with(|cell| {
            let Self {
                stack,
                index,
                count,
            } = &mut *cell.borrow_mut();
            if *count == 1 {
                if let Some(o) = TEMP::take_original_relative_path() {
                    global_ui_mut().relative = o;
                }
            }
            stack.truncate(*index + 1);
            *index += 1;
            *count += 1;

            log::debug!("Pushed: {pane:?}");
            stack.push(pane);
        });
    }

    pub fn stack_prev() -> bool {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            if *index > 0 {
                *index -= 1;
                true
            } else {
                false
            }
        })
    }

    pub fn stack_next() -> bool {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            if *index < stack.len() - 1 {
                *index += 1;
                true
            } else {
                false
            }
        })
    }

    pub fn current() -> FsPane {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            stack[*index].clone()
        })
    }

    pub fn swap_history() {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            let c = &stack[*index];
            // If we are in history, and we were in the other history, switch back to it
            if *index > 0
                && matches!(
                    stack[*index - 1],
                    FsPane::Files { .. } | FsPane::Folders { .. }
                )
                && matches!(c, FsPane::Files { .. } | FsPane::Folders { .. })
                && &stack[*index - 1] != c
            {
                *index -= 1;
                return;
            }

            // otherwise, create a new pane: folders unless we are already in it.
            let folders = !matches!(stack[*index], FsPane::Folders { .. });
            let pane = FsPane::new_history(folders, FILTERS::sort().into());

            //push
            stack.truncate(*index + 1);
            *index += 1;

            log::debug!("Pushed: {pane:?}");
            stack.push(pane);
        });
    }

    pub fn with_current_mut<R, F: FnOnce(&mut FsPane) -> R>(f: F) -> R {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            f(&mut stack[*index])
        })
    }

    pub fn with_current<R, F: FnOnce(&FsPane) -> R>(f: F) -> R {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            f(&stack[*index])
        })
    }

    /// Return the cwd for Nav/Custom/Fd
    pub fn cwd() -> Option<AbsPath> {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            for s in stack[0..=*index].iter().rev() {
                match s {
                    FsPane::Files { .. } | FsPane::Folders { .. } => {}
                    FsPane::Nav { cwd, .. }
                    | FsPane::Custom { cwd, .. }
                    | FsPane::Fd { cwd, .. } => {
                        return Some(cwd.clone());
                    }
                    _ => return None,
                }
            }
            if matches!(stack[*index], FsPane::Files { .. } | FsPane::Folders { .. }) {
                current_dir().ok().map(AbsPath::new_unchecked)
            } else {
                None
            }
        })
    }
    pub fn nav_cwd() -> Option<AbsPath> {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            if let FsPane::Nav { cwd, .. } = &stack[*index] {
                Some(cwd.clone())
            } else {
                None
            }
        })
    }

    pub fn save_input(
        content: String,
        cursor: u32,
    ) {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            match &mut stack[*index] {
                FsPane::Custom { input, .. } | FsPane::Nav { input, .. } => {
                    *input = (content, cursor)
                }
                _ => {}
            }
        });
    }

    pub fn has_saved_input() -> bool {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            match &stack[*index] {
                FsPane::Custom { input, .. }
                | FsPane::Nav { input, .. }
                | FsPane::Fd { input, .. }
                | FsPane::Stream { input, .. }
                | FsPane::Rg { input, .. }
                | FsPane::Files { input, .. }
                | FsPane::Folders { input, .. } => !(input.0.is_empty() && input.1 == 0),
                _ => false,
            }
        })
    }

    pub fn get_maybe_input() -> Option<(String, u32)> {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            match &stack[*index] {
                FsPane::Custom { input, .. } | FsPane::Nav { input, .. } => Some(input.clone()),
                _ => None,
            }
        })
    }

    pub fn take_maybe_input() -> Option<(String, u32)> {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            match &mut stack[*index] {
                FsPane::Custom { input, .. } | FsPane::Nav { input, .. } => {
                    Some(std::mem::take(input))
                }
                _ => None,
            }
        })
    }

    pub fn populate(
        injector: FsInjector,
        callback: impl FnOnce() + SSS,
    ) {
        let cfg = GLOBAL::with_cfg(|c| c.clone());
        Self::with_current(|pane| {
            let msg = match &pane {
                FsPane::Nav { cwd, .. } | FsPane::Custom { cwd, .. } => {
                    WatcherMessage::Switch(cwd.inner(), notify::RecursiveMode::NonRecursive)
                }
                FsPane::Fd { cwd, .. } | FsPane::Rg { cwd, .. } => {
                    // reload on small sizes?
                    WatcherMessage::Pause
                    // WatcherMessage::Switch(cwd.inner())
                }
                _ => WatcherMessage::Pause,
            };
            GLOBAL::send_watcher(msg);
            pane.populate(injector, &cfg, callback);
        });
    }
}
