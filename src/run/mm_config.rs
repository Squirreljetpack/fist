use cba::{_dbg, bo::load_type_or_default};
use matchmaker::{
    binds::BindMap,
    config::{
        DisplayConfig, OverlayConfig, Percentage, PreviewSetting, RenderConfig, RowConnectionStyle,
        TerminalConfig, TerminalLayoutSettings,
    },
    nucleo::nucleo,
};
use matchmaker_partial::Apply;
use ratatui::style::Modifier;
use std::path::Path;

use super::{FsAction, binds::default_binds};

#[cfg(feature = "mm_overrides")]
use crate::cli::env::EnvOpts;

use crate::{
    config::Config,
    lessfilter::Preset,
    ui::{
        confirm_overlay::ConfirmConfig, filters_overlay::FiltersConfig, menu_overlay::MenuConfig,
        prompt_overlay::PromptConfig, stash_overlay::StashConfig,
    },
};
use fist_types::When;

// ------- MM config --------

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
    pub stash: StashConfig,
    #[serde(default)]
    pub scratch: StashConfig,
    #[serde(default)]
    pub filters: FiltersConfig,
    #[serde(default)]
    pub prompt: PromptConfig,
    #[serde(default)]
    pub menu: MenuConfig,
    #[serde(default)]
    pub confirm: ConfirmConfig,
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

// -------------------------------------------------------------------------------------------

pub fn get_mm_cfg(
    path: &Path,
    cfg: &Config,
) -> MMConfig {
    let mut mm_cfg: MMConfig = load_type_or_default(path, |s| toml::from_str(s));
    #[cfg(feature = "mm_overrides")]
    if let Some(partial) = EnvOpts::get_mm_partial() {
        mm_cfg.render.apply(partial);
    }

    let binds = default_binds();
    default_binds().extend(mm_cfg.binds);
    mm_cfg.binds = binds;

    // Render display
    let RenderConfig {
        ui: _,
        query: _,
        results,
        status,
        preview,
        footer,
        header: _,
    } = &mut mm_cfg.render;

    // disable some configuration settings for consistency
    results.multi_prefix = results.multi_prefix.chars().next().unwrap_or('▌').into(); // single width
    results.right_align_last = true;
    results.stacked_columns = false;
    results.separator = Default::default();
    results.min_wrap_width = results.min_wrap_width.max(10);
    results.autoscroll.initial_preserved = 5;
    if cfg.global.mm.reverse.is_some() {
        results.reverse = cfg.global.mm.reverse
    }

    if status.template.is_empty() {
        status.template = r#"\m/\t"#.to_string();
    }

    *footer = DisplayConfig {
        style: Default::default(),
        wrap: true,
        row_connection: RowConnectionStyle::Full,
        ..Default::default()
    };

    // Preview display

    preview.initial.index = None;
    preview.initial.percentage = Percentage::new(70);

    let command = Preset::Preview.to_command_string(When::Auto);
    if preview.layout.is_empty() {
        preview.layout = vec![PreviewSetting {
            command,
            ..Default::default()
        }]
    } else if preview.layout[0].command.is_empty() {
        preview.layout[0].command = command
    }

    let tui = &mut mm_cfg.tui;
    // non-fullscreen by default
    if cfg.global.mm.fullscreen {
        tui.layout = None
    } else if tui.layout.is_none() {
        tui.layout = Some(TerminalLayoutSettings {
            percentage: Percentage::new(60),
            ..Default::default()
        })
    }
    #[cfg(debug_assertions)]
    {
        tui.clear_on_exit = false;
    }

    if let Err(p) = mm_cfg.filters.base.border {
        let mut full = mm_cfg.overlay.border.clone();
        full.apply(p);
        mm_cfg.filters.base.border = Ok(full)
    }
    if let Err(p) = mm_cfg.menu.border {
        let mut full = mm_cfg.overlay.border.clone();
        full.apply(p);
        mm_cfg.menu.border = Ok(full)
    }
    if let Err(p) = mm_cfg.stash.border {
        let mut full = mm_cfg.overlay.border.clone();
        full.apply(p);
        mm_cfg.stash.border = Ok(full)
    }
    if let Err(p) = mm_cfg.scratch.border {
        let mut full = mm_cfg.overlay.border.clone();
        full.apply(p);
        mm_cfg.scratch.border = Ok(full)
    }
    if let Err(p) = mm_cfg.confirm.border {
        let mut full = mm_cfg.overlay.border.clone();
        if p.modifier.is_none() {
            full.modifier = Modifier::ITALIC
        }
        full.apply(p);
        mm_cfg.confirm.border = Ok(full)
    }

    _dbg!(&mm_cfg);
    log::debug!("{mm_cfg:?}");

    mm_cfg
}
