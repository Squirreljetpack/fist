#![allow(warnings)]
use std::collections::HashMap;

use cli_boilerplate_automation::define_collection_wrapper;

use crate::lessfilter::file_rule::FileRuleKind;

define_collection_wrapper!(
    #[derive(Debug, serde::Deserialize, serde::Serialize)]
    MenuActions: HashMap<String, MenuAction>
);
// todo: custom deserialize impl

/// A menu action is activated through the [`crate::ui::menu_overlay::MenuOverlay`], and executes a user-defined script.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MenuAction {
    script: String,
    #[serde(default)]
    exec: ExecuteType,
    #[serde(default, skip)]
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone)]
pub enum Condition {
    FileRule(FileRuleKind),
}

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
pub enum ExecuteType {
    Preview {
        stdout: bool,
        stderr: bool,
    },
    Execute {
        wait: bool,
    },
    #[default]
    Detach,
    // todo: Queue
}
