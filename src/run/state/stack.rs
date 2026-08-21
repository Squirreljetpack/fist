#![allow(clippy::upper_case_acronyms)]
#![allow(unused_variables)]

use std::{cell::RefCell, mem::discriminant, sync::RwLock};

use log::{self};
use matchmaker::SSS;

use crate::{
    abspath::AbsPath,
    run::{
        state::{InitialNoRelative, FILTERS, GLOBAL, STORE},
        FsInjector, FsPane,
    },
    watcher::WatcherMessage,
};

thread_local! {
    static STACK: RefCell<STACK> = const { RefCell::new(STACK::new()) }
}

/// The cwd used by [`crate::run::item::PathItem::render`] for relative-path
/// display. Set once per populate ([`STACK::populate`]); `None` disables
/// relative display (initial pane, or no cwd).
///
/// Replaces the per-render `STACK::cwd()` lookup: render runs on background
/// threads (injector/worker) that do not share the main thread's `STACK`
/// thread-local, so the value is snapshotted here on the main thread instead.
static RENDER_PATH: RwLock<Option<AbsPath>> = RwLock::new(None);

/// Set the render cwd for the ongoing populate. `None` disables relative
/// path display (see [`render_path`]).
pub fn set_render_path(path: Option<AbsPath>) {
    if let Ok(mut lock) = RENDER_PATH.write() {
        *lock = path;
    }
}

/// The render cwd for the ongoing populate, or `None` (no relative display).
pub fn render_path() -> Option<AbsPath> {
    RENDER_PATH.read().ok().and_then(|g| g.clone())
}

pub struct STACK {
    stack: Vec<FsPane>, // invariants: nonempty
    index: usize,
}
impl STACK {
    const fn new() -> Self {
        Self {
            stack: Vec::new(),
            index: 0,
        }
    }

    pub fn init(pane: FsPane) {
        STACK.with(|s| {
            *s.borrow_mut() = Self {
                stack: vec![pane],
                index: 0,
            }
        });
    }

    pub fn len() -> usize {
        STACK.with(|s| s.borrow().stack.len())
    }

    pub fn push(pane: FsPane) -> bool {
        STACK.with(|cell| {
            let Self { stack, index } = &mut *cell.borrow_mut();
            stack.truncate(*index + 1);

            let same = discriminant(&stack[*index]) == discriminant(&pane);

            match stack[*index] {
                FsPane::Find { .. } => {
                    if GLOBAL::cfg().panes.find.on_leave_unset_dirs_only {
                        FILTERS::with_vis_mut(|v| v.dirs = false);
                    }
                }
                _ => {}
            };

            *index += 1;

            log::debug!("Pushed: {pane:?}");
            stack.push(pane);

            same
        })
    }

    pub fn stack_prev() -> bool {
        STACK.with(|cell| {
            let Self { index, .. } = &mut *cell.borrow_mut();
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

    /// returns true if newly entering history pane
    pub fn swap_history() -> bool {
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
                return false;
            }

            // otherwise, create a new pane: folders unless we are already in it.
            let ret = !matches!(stack[*index], FsPane::Folders { .. } | FsPane::Files { .. });
            let folders = !matches!(stack[*index], FsPane::Folders { .. });
            let pane = FsPane::new_history(folders);

            //push
            stack.truncate(*index + 1);
            *index += 1;

            log::debug!("Pushed: {pane:?}");
            stack.push(pane);

            ret
        })
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

    /// Whether changing the current pane's sort requires repopulating it.
    pub fn reloads_by_sorting() -> bool {
        Self::with_current(|pane| {
            matches!(
                pane,
                FsPane::Files { .. }
                    | FsPane::Folders { .. }
                    | FsPane::Apps { .. }
                    | FsPane::Search { .. }
            )
        })
    }

    /// Returns whether it pushed (pane type is different)
    pub fn set_or_push(pane: FsPane) -> bool {
        let different_type = Self::with_current(|current| current != &pane);

        if !different_type {
            // update current in place
            Self::with_current_mut(|p| *p = pane);
        } else {
            STACK::push(pane);
        }

        different_type
    }

    // pub fn with_previous<R, F>(f: F) -> Option<R>
    // where
    // F: FnOnce(&FsPane, bool) -> R,
    // {
    //     STACK.with(|cell| {
    //         let borrowed = cell.borrow();
    //         let Self { stack, index, .. } = &*borrowed;

    //         if *index > 0 {
    //             let current = &stack[*index];
    //             let prev = &stack[*index - 1];
    //             let same_variant = discriminant(prev) == discriminant(current);

    //             Some(f(prev, same_variant))
    //         } else {
    //             None
    //         }
    //     })
    // }

    pub fn populate(
        injector: FsInjector,
        callback: impl FnOnce() + SSS,
    ) {
        // the render cwd for this populate: the initial pane (index 0) with
        // the InitialNoRelative marker renders without one — path.relative
        // never triggers — otherwise the stack cwd
        let no_relative = STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            *index == 0 && STORE::contains::<InitialNoRelative>()
        });
        set_render_path(if no_relative { None } else { Self::cwd() });

        let cfg = GLOBAL::cfg().clone();
        Self::with_current(|pane| {
            let msg = match &pane {
                FsPane::Nav { cwd, .. } | FsPane::Custom { cwd, .. } => {
                    WatcherMessage::Switch(cwd.inner(), notify::RecursiveMode::NonRecursive)
                }
                FsPane::Find { .. } | FsPane::Search { .. } => {
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

// ---------------- utilities
impl STACK {
    /// Return the cwd for Nav/Custom/Fd
    pub fn cwd() -> Option<AbsPath> {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            if stack.is_empty() {
                return None;
            }
            let mut seen = false;
            for s in stack[0..=*index].iter().rev() {
                match s {
                    FsPane::Files { .. } | FsPane::Folders { .. } | FsPane::Stash { .. } => {
                        seen = true
                    }
                    FsPane::Nav { cwd, .. }
                    | FsPane::Custom { cwd, .. }
                    | FsPane::Find { cwd, .. }
                    | FsPane::Search { cwd, .. } => {
                        return Some(cwd.clone());
                    }
                    FsPane::Apps { .. } => return None,
                }
            }

            // FsPane::Files looks for the last directory, or else the original
            seen.then_some(AbsPath::initial())
        })
    }

    /// Corresponds to the cwd displayed in prompt.
    pub fn _cwd() -> AbsPath {
        STACK::cwd().unwrap_or(AbsPath::initial())
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

    pub fn is_last() -> bool {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &*cell.borrow();
            *index == stack.len() - 1
        })
    }

    // don't save index for rg and find because order is not guaranteed
    /// Note that because state is saved on update (and initialization) for [`FsPane::Search`] (see [`crate::run::ahandlers::fs_reload`]), rg is omitted here
    // todo: lowpri: configurable save
    pub fn save_input(
        content: String,
        cursor: u32,
    ) {
        STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            match &mut stack[*index] {
                FsPane::Custom { input, .. }
                | FsPane::Nav { input, .. }
                | FsPane::Files { input, .. }
                | FsPane::Folders { input, .. }
                | FsPane::Stash { input, .. } => {
                    log::debug!("saving: {content} {cursor}");
                    *input = (content, cursor)
                }
                FsPane::Find { input, .. } => {
                    // input.0 = content,
                    *input = (content, cursor)
                }
                _ => {}
            }
        });
    }

    // only restore index of nav and custom panes (is this what we want?)
    // see also [FsPane::get_input]
    pub fn take_maybe_index() -> Option<u32> {
        let i = STACK.with(|cell| {
            let Self { stack, index, .. } = &mut *cell.borrow_mut();
            match &mut stack[*index] {
                FsPane::Custom { input, .. }
                | FsPane::Find { input, .. }
                | FsPane::Search { input, .. }
                | FsPane::Nav { input, .. }
                | FsPane::Files { input, .. }
                | FsPane::Folders { input, .. }
                | FsPane::Stash { input, .. } => {
                    let ret = std::mem::take(&mut input.1);
                    // 0 -> None because we only store index
                    (ret != 0).then_some(ret)
                }
                _ => None,
            }
        });

        log::trace!("Took stashed index {i:?}");

        i
    }

    pub fn in_app() -> bool {
        STACK::with_current(|x| matches!(x, FsPane::Apps { .. }))
    }
    pub fn in_rg() -> bool {
        STACK::with_current(|x| matches!(x, FsPane::Search { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_cwd() {
        let p1 = AbsPath::new_unchecked(Path::new("/tmp/test_dir1"));
        let p2 = AbsPath::new_unchecked(Path::new("/tmp/test_dir2"));

        STACK::init(FsPane::Nav {
            cwd: p1.clone(),
            sort: Default::default(),
            vis: Default::default(),
            depth: 1,
            input: (String::new(), 0),
            complete: Default::default(),
        });

        assert_eq!(STACK::cwd(), Some(p1.clone()));

        // Push new pane
        STACK::push(FsPane::Nav {
            cwd: p2.clone(),
            sort: Default::default(),
            vis: Default::default(),
            depth: 1,
            input: (String::new(), 0),
            complete: Default::default(),
        });

        assert_eq!(STACK::cwd(), Some(p2.clone()));

        // the thread-local stack is main-thread only: `STACK::cwd()` on a
        // background thread sees an empty stack (the ACTIVE_CWD sync was
        // replaced by RENDER_PATH — see set_render_path/populate)
        let bg_cwd = std::thread::spawn(STACK::cwd).join().unwrap();
        assert_eq!(bg_cwd, None);
    }
}
