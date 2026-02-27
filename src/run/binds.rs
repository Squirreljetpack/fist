use matchmaker::{
    action::Action,
    bindmap,
    binds::{BindMap, BindMapExt, key},
};

use crate::lessfilter::Preset;
use fist_types::When;

use super::FsAction;

pub fn default_binds() -> BindMap<FsAction> {
    let mut fs = bindmap!(
        // Nav
        // ----------------------------------
        key!(up) => Action::Up(1),
        key!(down) => Action::Down(1),
        key!(shift-right) => Action::ForwardChar,
        key!(shift-left) => Action::BackwardChar,
        key!(enter) => Action::Accept,
        key!(alt-enter) => Action::Print("".into()),
        key!(tab) => [Action::Toggle, Action::Down(1)],
        key!(ctrl-a) => Action::CycleAll,

        key!(right) => FsAction::Advance,
        key!(left) => FsAction::Parent,
        key!(ctrl-f) => FsAction::Find,
        key!(ctrl-r) => FsAction::Search,
        key!(ctrl-g) => FsAction::History,
        key!(ctrl-z) => FsAction::Undo,
        key!(alt-z) => FsAction::Redo,
        key!(alt-a), key!(alt-shift-a) => FsAction::App,
        key!(ctrl-shift-'z') => FsAction::Redo,
        key!(ctrl-'/') => FsAction::Jump("".into(), None),

        // Display
        // ----------------------------------
        key!(ctrl-t) => FsAction::Stash,
        key!(alt-shift-t) => FsAction::ClearStash(None),
        key!(alt-shift-s) => FsAction::ClearStash(Some(true)), // clear custom
        key!(ctrl-e) => FsAction::Menu,
        // -- filters --
        key!(ctrl-i) => FsAction::Filters,
        key!(ctrl-d) => FsAction::FsToggle,
        key!(alt-h), key!(ctrl-s) => FsAction::ToggleHidden,

        // file actions
        // ----------------------------------
        key!(ctrl-y) => FsAction::CopyPath,
        key!(delete) => FsAction::Trash,
        key!(shift-delete) => FsAction::Delete,

        key!(ctrl-v) => FsAction::Paste("".into()),

        // these behave the same on the prompt
        key!(ctrl-x) => FsAction::Cut,
        key!(ctrl-c) => FsAction::Copy,
        key!(ctrl-n) => FsAction::New,
        key!(alt-s) => FsAction::Push,

        // preview and execution
        // ----------------------------------

        // preview
        key!('?') => Action::Preview(Preset::Preview.to_command_string(When::Auto)),
        // Verbose information
        key!(alt - '/') => Action::Preview(Preset::Info.to_command_string(When::Auto)),
        // Quick Look + a header
        key!(alt - shift - '/') => Action::Preview(Preset::Display.to_command_string(When::Always)),
        // Keybind help
        key!(ctrl-shift-h), key!(shift-cmd-h) => FsAction::help(),

        // This one acts on the parent directory if a file is selected.
        // Note that the "Directory" action in the edit preset defers to $VISUAL.
        key!(ctrl-q), key!(ctrl-'.')  => FsAction::new_lessfilter(Preset::Edit, false),
        key!(ctrl-enter) => FsAction::new_lessfilter(Preset::Open, false),
        // A free preset for the user to decide what to do with
        key!(alt-8) => FsAction::new_lessfilter(Preset::Alternate, true),
        // Maximize preview
        // true => display the output in a pager
        key!(ctrl-l) => FsAction::Execute("eval \"$=FS_PREVIEW_COMMAND\"".to_string(), 1),
        // For "full" or interactive terminal output
        key!(alt-l)  => FsAction::new_lessfilter(Preset::Extended, true),

        // open a shell in the current directory
        key!(ctrl-esc) => Action::Execute("$SHELL".into()),
        // Actions can be defined directly as a ::Execute keybind, as a [menu] action, or indirectly through [stack.cas]

        // misc
        // ---------------------------------------
        key!(shift-up) => Action::PreviewUp(1),
        key!(shift-down) => Action::PreviewDown(1),

        key!(ctrl-shift-'/'), key!(shift-cmd-'/') => Action::CyclePreview,
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
    // cmd+backspace is traditional for trash on mac
    #[cfg(target_os = "macos")]
    let ext = bindmap!(
        key!(ctrl-h), key!(cmd-backspace) => FsAction::Trash,
        key!(alt-backspace) => Action::DeleteWord,
        key!(ctrl-shift-backspace), key!(shift-cmd-backspace) => FsAction::Delete,
    );
    #[cfg(not(target_os = "macos"))]
    let ext = bindmap!();

    fs.extend(ext);

    let mut base = BindMap::default_binds();
    base.extend(fs);

    base
}
