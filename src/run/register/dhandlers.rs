use std::{
    ffi::OsString,
    process::{Command, Stdio},
};

use cba::{
    bog::BogOkExt,
    broc::{tty_or_inherit, CommandExt, EnvVars, SHELL},
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
    clipboard,
    run::{
        ahandlers::fs_reload,
        item::PathItem,
        pane::FsPane,
        selection,
        state::{sort, ExecuteHandlerShouldProcessParent, FILTERS, GLOBAL, STACK, STORE, TASKS},
        FsMatchmaker,
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
