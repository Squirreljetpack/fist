use cli_boilerplate_automation::bother::types::When;
use matchmaker::{
    action::Action,
    bindmap,
    binds::{BindMap, key},
};

use crate::lessfilter::Preset;

use super::FsAction;

pub fn default_binds() -> BindMap<FsAction> {
    let ret = bindmap!(
        // Nav
        // ----------------------------------
        key!(up) => Action::Up(1),
        key!(down) => Action::Down(1),
        key!(shift-right) => Action::ForwardChar,
        key!(shift-left) => Action::BackwardChar,
        key!(enter) => Action::Accept,
        key!(ctrl-enter), key!(alt-enter) => Action::Print("".into()),
        key!(tab) => [Action::Toggle, Action::Down(1)],
        key!(ctrl-a) => Action::CycleAll,

        key!(right) => FsAction::Advance,
        key!(left) => FsAction::Parent,
        key!(ctrl-f) => FsAction::Find,
        key!(ctrl-r) => FsAction::Rg,
        key!(ctrl-g) => FsAction::History,
        key!(ctrl-z) => FsAction::Undo,
        key!(alt-z) => FsAction::Redo,
        key!(ctrl-shift-'z') => FsAction::Redo,
        key!('~') => FsAction::Jump("".into(), Some('~')),
        key!('/') => FsAction::Jump("".into(), Some('/')), // doesn't make the most sense but its convenient

        // Display
        // ----------------------------------
        key!(ctrl-s) => FsAction::Stash,
        key!(alt-shift-s) => FsAction::ClearStash,
        key!(ctrl-e) => FsAction::Menu,
        // -- filters --
        key!(alt-f), key!(ctrl-shift-f) => FsAction::Filters,
        key!(ctrl-d) => FsAction::ToggleDirs,
        key!(ctrl-h), key!(alt-h) => FsAction::ToggleHidden,

        // file actions
        // ----------------------------------
        key!(ctrl-y) => FsAction::CopyPath,
        key!(delete) => FsAction::Trash,
        key!(shift-delete) => FsAction::Delete,
        key!(ctrl-v) => FsAction::Paste("".into()),
        key!(alt-b) => FsAction::Backup,

        // these behave the same on the prompt
        key!(ctrl-x) => FsAction::Cut,
        key!(ctrl-c) => FsAction::Copy,
        key!(ctrl-n) => FsAction::New,

        // preview
        key!('?') => Action::Preview(Preset::Preview.to_command_string(When::Auto)),
        key!(alt - '/') => Action::Preview(Preset::Display.to_command_string(When::Always)),
        key!(ctrl-shift-h), key!(alt-shift-h) => Action::Help("".into()),
        // spawning
        key!(alt-s) => Action::Execute("$SHELL".into()),

        // lessfilter
        key!(ctrl-b) => FsAction::Lessfilter { preset: Preset::Open, paging: false, header: When::Auto },
        key!(alt-b) => FsAction::Lessfilter{ preset: Preset::Edit, paging: false, header: When::Auto},
        key!(ctrl-l) => FsAction::Lessfilter{ preset: Preset::Preview, paging: true, header: When::Auto},
        key!(alt-l) => FsAction::Lessfilter{ preset: Preset::Extended, paging: true, header: When::Auto},

        // misc
        // ---------------------------------------
        key!(shift-up) => Action::PreviewUp(1),
        key!(shift-down) => Action::PreviewDown(1),

        key!(ctrl-shift-'/') => Action::CyclePreview,
        key!(alt-r) => Action::Reload("".to_string()),
        key!(ctrl-0), key!(ctrl-'`') => FsAction::AutoJump(0),
        key!(ctrl-1) => FsAction::AutoJump(1),
        key!(ctrl-2) => FsAction::AutoJump(2),
        key!(ctrl-3) => FsAction::AutoJump(3),
        key!(ctrl-4) => FsAction::AutoJump(4),
        key!(ctrl-5) => FsAction::AutoJump(5),
        key!(ctrl-6) => FsAction::AutoJump(6),
        key!(ctrl-7) => FsAction::AutoJump(7),
        key!(ctrl-8) => FsAction::AutoJump(8),
        key!(ctrl-9) => FsAction::AutoJump(9),
    );
    ret
}
