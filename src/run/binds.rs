use matchmaker::{
    action::Action,
    bindmap,
    binds::{key, BindMap, BindMapExt},
};

use crate::lessfilter::Preset;
use fist_types::When;

use super::{queue::QueueSelector, FsAction};

pub fn default_binds() -> BindMap<FsAction> {
    let mut fs = bindmap!(
        // hidden defaults
        key!(shift-up) => Action::PreviewUp(1),
        key!(shift-down) => Action::PreviewDown(1),
        key!(ctrl-'/') => Action::NextPreview,
        key!(ctrl-shift-'/') => Action::PrevPreview,

        // previews (builtin because header is not exposed)
        // ----------------------------------
        // preview
        key!('?') => FsAction::LessfilterPreview(Preset::Preview, When::Auto),
        // Verbose information
        key!(alt - '/') => FsAction::LessfilterPreview(Preset::Info, When::Auto),
        // Quick Look + a header
        key!(alt - shift - '/') => FsAction::LessfilterPreview(Preset::Display, When::Always),
        // Keybind help
        key!(alt-h) => FsAction::help(),
        key!(ctrl-shift-backspace), key!(shift-cmd-backspace) => FsAction::Delete(false),
    );

    // cmd+backspace is traditional for trash on mac
    #[cfg(target_os = "macos")]
    let ext = bindmap!(
        key!(ctrl-h), key!(cmd-backspace) => FsAction::Trash(false),
        key!(ctrl-shift-backspace), key!(shift-cmd-backspace) => FsAction::Delete(false),
        key!(alt-backspace) => Action::DeleteWord,
    );

    // default binds for when without delete key
    #[cfg(not(target_os = "macos"))]
    let ext = bindmap!(
        key!(alt-backspace) => FsAction::Trash(false),
        key!(ctrl-shift-backspace), key!(shift-cmd-backspace) => FsAction::Delete(false),
    );

    fs.extend(ext);
    fs.extend_from(BindMap::default_binds());

    fs
}

#[allow(unused)]
/// mirrors settings in mm.toml
fn config_as_code() -> BindMap<FsAction> {
    bindmap!(
        // MM
        // ----------------------------------
        key!(right) => Action::ForwardChar,
        key!(left) => Action::BackwardChar,
        key!(tab) => [Action::ToggleSelection, Action::Down(1)],
        key!(alt-enter) => Action::Print("".into()),
        key!(alt-r) => Action::Reload("".to_string()),

        // Panes
        // ----------------------------------
        key!(shift-right) => FsAction::Advance,
        key!(shift-left) => FsAction::Parent,
        key!(ctrl-f) => FsAction::Find,
        key!(ctrl-r) => FsAction::Search,
        key!(ctrl-g) => FsAction::History,
        key!(ctrl-z) => FsAction::Undo,
        // key!(alt-a), key!(alt-shift-a) => FsAction::App,
        key!(alt-z), key!(ctrl-shift-'z') => FsAction::Redo,

        // Display
        // ----------------------------------
        key!(ctrl-u) => FsAction::ShowQueue,
        key!(ctrl-shift-u), key!(alt-shift-u) => FsAction::ClearQueue(QueueSelector::All, false),
        key!(alt-u) => Action::ClearQuery,
        key!(ctrl-e) => FsAction::ShowMenu,
        // -- options --
        key!(ctrl-p) => FsAction::ShowOptions,
        key!(ctrl-d) => FsAction::FsToggle,
        key!(ctrl-s) => FsAction::ToggleHidden,

        // File actions
        // ----------------------------------
        key!(ctrl-y) => FsAction::CopyPath,
        key!(delete) => FsAction::Trash(true),
        key!(shift-delete) => FsAction::Delete(false),
        key!(ctrl-shift-r) => FsAction::Rename,
        key!(ctrl-v) => FsAction::Paste("".into()),

        // these behave the same on the prompt
        key!(ctrl-x) => FsAction::Cut,
        key!(ctrl-c) => FsAction::Copy,
        key!(ctrl-n) => FsAction::New,

        // Stash
        // ----------------------------------
        // PushStash/OpenStash are bound by the user; the queue insertion
        // actions are Copy and Cut

        // Prompt
        // ----------------------------------
        key!(alt-space) => FsAction::LockPrompt(None), // None toggles the prompt

        // Autojump
        // --------------------------------
        key!(ctrl-0), key!(ctrl-enter) => FsAction::AutoJump(0),
        key!(ctrl-1) => FsAction::AutoJump(1),
        key!(ctrl-2) => FsAction::AutoJump(2),
        key!(ctrl-3) => FsAction::AutoJump(3),
        key!(ctrl-4) => FsAction::AutoJump(4),
        key!(ctrl-5) => FsAction::AutoJump(5),
        key!(ctrl-6) => FsAction::AutoJump(6),
        key!(ctrl-7) => FsAction::AutoJump(7),
        key!(ctrl-8) => FsAction::AutoJump(8),
        key!(ctrl-9) => FsAction::AutoJump(9),
    )
}
