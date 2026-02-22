use std::path::Path;

use cli_boilerplate_automation::bo::load_type_or_default;
use matchmaker::{
    binds::BindMap,
    config::{
        DisplayConfig, OverlayConfig, Percentage, PreviewSetting, RenderConfig, RowConnectionStyle,
        TerminalConfig, TerminalLayoutSettings,
    },
    nucleo::nucleo,
};

use super::{FsAction, binds::default_binds};
use crate::{
    config::Config,
    lessfilter::Preset,
    ui::{
        filters_overlay::FiltersConfig, menu_overlay::MenuConfig, prompt_overlay::PromptConfig,
        stash_overlay::StashConfig,
    },
};
use fist_types::When;

// ------- Main config --------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MMConfig {
    // configure the ui
    #[serde(default, flatten)]
    pub render: RenderConfig,
    /// Base overlay style
    #[serde(default)]
    pub overlay: OverlayConfig,

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
    } = &mut mm_cfg.render;

    results.multi_prefix = results.multi_prefix.chars().next().unwrap_or('▌').into(); // single width
    results.right_align_last = true;

    *footer = DisplayConfig {
        modifier: Default::default(),
        fg: Default::default(),
        wrap: true,
        row_connection_style: RowConnectionStyle::Full,
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
