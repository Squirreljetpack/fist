#![allow(clippy::upper_case_acronyms)]
#![allow(non_snake_case)]
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{abspath::AbsPath, run::item::PathItem, ui::menu_overlay::PromptKind};

pub mod TEMP {
    use super::*;

    thread_local! {
        static PREV_DIRECTORY: RefCell<Option<AbsPath>> = const { RefCell::new(None) };
        static STASHED_INDEX: RefCell<Option<u32>> = const { RefCell::new(None) };
        static INPUT_BAR_CONTENT: RefCell<(Option<PromptKind>, Result<PathItem, AbsPath>)> = const { RefCell::new((None, Err(AbsPath::empty()))) };
        static ORIGINAL_RELATIVE_PATH: RefCell<Option<bool>> = const { RefCell::new(None) };
    }
    static TEMP_BOOL: AtomicBool = AtomicBool::new(false);

    pub fn take_prev_dir() -> Option<AbsPath> {
        PREV_DIRECTORY.with_borrow_mut(|x| x.take())
    }
    pub fn set_prev_dir(path: Option<AbsPath>) {
        PREV_DIRECTORY.replace(path);
    }

    pub fn take_stashed_index() -> Option<u32> {
        STASHED_INDEX.with_borrow_mut(|i| i.take())
    }
    pub fn set_stashed_index(index: u32) -> Option<u32> {
        STASHED_INDEX.replace(Some(index))
    }

    pub fn take_input_bar() -> (Option<PromptKind>, Result<PathItem, AbsPath>) {
        INPUT_BAR_CONTENT
            .with_borrow_mut(|(p, s)| (p.take(), std::mem::replace(s, Err(AbsPath::empty()))))
    }

    /// If menu_prompt is set, menu starts an input overlay.
    ///
    /// The Ok variant of menu_target describes the target,
    /// while the Err variant corresponds to no target
    /// -- instead defining the cwd context, in which case
    /// only a restrictred subset of the menu actions is available.
    ///
    /// # Additional
    /// When the prompt is set and the target is Ok, the target's filename is shown in the title of the input bar.
    pub fn set_input_bar(
        menu_prompt: Option<PromptKind>,
        menu_target: Result<PathItem, AbsPath>,
    ) {
        let _ = INPUT_BAR_CONTENT.replace((menu_prompt, menu_target));
    }

    pub fn set_initial_relative_path(relative: bool) {
        ORIGINAL_RELATIVE_PATH.replace(Some(relative));
    }
    pub fn get_original_relative_path() -> Option<bool> {
        ORIGINAL_RELATIVE_PATH.with_borrow(|x| *x)
    }

    pub fn set_that_execute_handler_should_process_cwd() {
        TEMP_BOOL.store(true, Ordering::SeqCst);
    }
    pub fn take_whether_execute_handler_should_process_cwd() -> bool {
        TEMP_BOOL
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    // lowpri: drop here for now, alternative approach is mm to support dynamic rebinding
    pub static QUERY_RELOAD: AtomicBool = AtomicBool::new(false);
}
