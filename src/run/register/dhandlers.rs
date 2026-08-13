use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use cba::{
    bog::BogOkExt,
    broc::{CommandExt, EnvVars, SHELL, tty_or_inherit},
    env_vars,
};
use easy_ext::ext;
use log::{debug, info, warn};
use matchmaker::{
    message::{Event, Interrupt},
    preview::AppendOnly,
};
use tokio::io::AsyncReadExt;

use crate::{
    abspath::AbsPath,
    aliases::MMState,
    cli::paths::text_renderer_path,
    clipboard,
    run::{
        FsMatchmaker,
        ahandlers::fs_reload,
        item::PathItem,
        pane::FsPane,
        selection,
        state::{ExecuteHandlerShouldProcessParent, FILTERS, GLOBAL, STACK, STORE, TASKS, sort},
    },
    utils::formatter::format_path,
};
use fist_types::filters::SortOrder;

// ------------------------------------------------------------------------
// Execution-mode transport
// ------------------------------------------------------------------------

// ------------------------------------------------------------------------

// ------------------------------------------------------------------------
// Execution helpers
// ------------------------------------------------------------------------

// ------------------------------------------------------------------------
