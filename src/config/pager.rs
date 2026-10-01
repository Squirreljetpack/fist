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

    /// Footer prompt text shown by the pager.
    pub prompt: Option<String>,

    /// Always enable horizontal scrolling.
    pub horizontal_scroll: bool,

    /// Smart case search: queries with no uppercase characters match case-
    /// insensitively, queries containing uppercase stay case-sensitive.
    pub smart_case: bool,

    /// When true, appends `--style=+changes` or `--style=-changes` to bat args
    /// based on whether the file has unstaged git changes.
    pub smart_changes: bool,

    /// Command + args to run when given a directory path.
    /// `{}` element is replaced with the directory path at invocation time.
    /// Empty vec disables the branch — directories pass through to bat.
    pub display_directory: Vec<String>,

    /// Start the pager in follow mode (auto-scroll as new output arrives).
    /// Set by the `+F` CLI arg, not the config file.
    #[serde(skip)]
    pub follow: bool,
}

impl Default for PagerConfig {
    fn default() -> Self {
        Self {
            bat_opts: Some(vec!["--color=always".into(), "--style=changes".into()]),
            line_numbers: false,
            follow: false,
            prompt: None,
            horizontal_scroll: false,
            smart_case: true,
            smart_changes: false,
            display_directory: vec![
                "fs".into(),
                ":tool".into(),
                "liza".into(),
                ":u2".into(),
                "--".into(),
                "{}".into(),
            ],
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
