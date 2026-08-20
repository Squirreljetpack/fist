use std::sync::LazyLock;

use crate::cli::paths::pager_cfg_path;

/// Configure the pager (bat passthrough into minus).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PagerConfig {
    /// Bat passthrough args.
    /// `None` disables bat entirely (raw stream into the pager).
    pub bat_opts: Option<Vec<String>>,

    /// Show line numbers in the pager.
    pub line_numbers: bool,

    /// Start the pager in follow mode (auto-scroll as new output arrives).
    pub follow: bool,

    /// Footer prompt text shown by the pager.
    pub prompt: Option<String>,

    /// Always enable horizontal scrolling.
    pub horizontal_scroll: bool,

    /// Smart case search: queries with no uppercase characters match case-
    /// insensitively, queries containing uppercase stay case-sensitive.
    pub smart_case: bool,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            bat_opts: Some(vec!["--color=always".into(), "--style=changes".into()]),
            line_numbers: false,
            follow: false,
            prompt: None,
            horizontal_scroll: false,
            smart_case: true, // nonstandard in the terminal but conventional in modern apps
        }
    }
}

static PAGER_CFG: LazyLock<PagerConfig> = LazyLock::new(|| {
    let cfg = std::fs::read_to_string(pager_cfg_path()).ok();
    match cfg.as_deref().and_then(|s| toml::from_str(s).ok()) {
        Some(cfg) => cfg,
        None => {
            log::error!(
                "Failed to parse pager config at {}; using defaults",
                pager_cfg_path().display()
            );
            PagerConfig::default()
        }
    }
});

/// Pager config from pager_cfg_path()
pub fn pager_cfg() -> &'static PagerConfig {
    &PAGER_CFG
}
