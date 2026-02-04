use std::path::Path;

use cli_boilerplate_automation::{bo::load_type_or_default, bother::enums::When};
use matchmaker::{
    action::Action,
    bindmap,
    binds::{BindMap, key},
    config::{
        DisplayConfig, OverlayConfig, Percentage, PreviewSetting, RenderConfig, TerminalConfig,
        TerminalLayoutSettings,
    },
    nucleo::nucleo,
};

use crate::{
    config::Config,
    lessfilter::Preset,
    run::action::FsAction,
    ui::{
        filters_overlay::FiltersConfig, menu_overlay::MenuConfig, prompt_overlay::PromptConfig,
        stash_overlay::StashConfig,
    },
};

// ------- Main config --------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MMConfig {
    // configure the ui
    #[serde(default, flatten)]
    pub render: RenderConfig,

    // overlays
    #[serde(default)]
    pub scratch: StashConfig,
    #[serde(default)]
    pub filters: FiltersConfig,
    #[serde(default)]
    pub prompt: PromptConfig,
    #[serde(default)]
    pub menu: MenuConfig,
    #[serde(default)]
    pub tui: TerminalConfig,

    // binds
    #[serde(default)]
    pub binds: BindMap<FsAction>,
}

impl Default for MMConfig {
    fn default() -> Self {
        toml::from_str(include_str!("../../assets/config/mm.toml")).unwrap()
    }
}

pub const MATCHER_CONFIG: nucleo::Config = const { nucleo::Config::DEFAULT.match_paths() };

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
        key!(ctrl-b) => FsAction::Display(Preset::Open, false, When::Auto),
        key!(alt-b) => FsAction::Display(Preset::Edit, false, When::Auto),
        // display
        key!(ctrl-l) => FsAction::Display(Preset::Preview, true, When::Auto),
        key!(alt-l) => FsAction::Display(Preset::Extended, true, When::Auto),


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

pub fn get_mm_cfg(
    path: &Path,
    cfg: &Config,
) -> MMConfig {
    let mut mm_cfg: MMConfig = load_type_or_default(path, |s| toml::from_str(s));

    let binds = default_binds();
    default_binds().append(&mut mm_cfg.binds);
    mm_cfg.binds = binds;

    // Render display
    let RenderConfig {
        ui,
        input,
        results,
        preview,
        footer,
        header,
        overlay,
    } = &mut mm_cfg.render;

    results.multi_prefix = results.multi_prefix.chars().next().unwrap_or('▌').into(); // single width
    results.right_align_last = true;

    *footer = DisplayConfig {
        match_indent: true,
        modifier: Default::default(),
        fg: Default::default(),
        ..Default::default()
    };

    // Preview display
    let default_command = Preset::Preview.to_command_string(When::Auto);
    if preview.layout.len() <= 1 {
        let (layout, command) = if let Some(p) = preview.layout.pop() {
            (
                p.layout,
                if p.command.is_empty() {
                    default_command
                } else {
                    p.command
                },
            )
        } else {
            (Default::default(), default_command)
        };

        preview.layout = vec![PreviewSetting { layout, command }]
    }

    // Overlay
    mm_cfg.render.overlay = Some(mm_cfg.render.overlay.unwrap_or(OverlayConfig {
        outer_dim: false,
        ..Default::default()
    }));

    // non-fullscreen by default
    if mm_cfg.tui.layout.is_none() {
        mm_cfg.tui.layout = Some(TerminalLayoutSettings {
            percentage: Percentage::new(60),
            ..Default::default()
        })
    }

    log::debug!("{mm_cfg:?}");
    mm_cfg
}
