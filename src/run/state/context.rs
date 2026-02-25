#![allow(unused)]

use matchmaker::preview::AppendOnly;
pub struct ActionContext {
    // pub execute_handler_should_process_cwd: bool,
    // pub bind_tx: BindSender<FsAction>,
    pub print_handle: AppendOnly<String>,
}

impl ActionContext {
    pub fn new(print_handle: AppendOnly<String>) -> Self {
        Self { print_handle }
    }
}
