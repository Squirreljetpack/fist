use std::sync::OnceLock;

use crate::config::ui::StyleConfig;
use ratatui::style::Style;

static PATH_DISPLAY_CONFIG: OnceLock<StyleConfig> = OnceLock::new();

pub fn global_ui_init(style_cfg: StyleConfig) {
    PATH_DISPLAY_CONFIG
        .set(style_cfg)
        .expect("global UI config initialized more than once");
}
pub fn global_ui() -> &'static StyleConfig {
    PATH_DISPLAY_CONFIG
        .get()
        .expect("global UI config not initialized")
}

/// The UI config when initialized; `None` before [`global_ui_init`] (e.g. in
/// unit tests that fire toasts).
pub fn try_global_ui() -> Option<&'static StyleConfig> {
    PATH_DISPLAY_CONFIG.get()
}

pub fn prompt_main_style() -> Style {
    global_ui().raw_prompt_style.into()
}
