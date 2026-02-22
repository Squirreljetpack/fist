use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::config::ui::StyleConfig;

static PATH_DISPLAY_CONFIG: RwLock<StyleConfig> = RwLock::new(StyleConfig::DEFAULT);
pub fn global_ui_init(style_cfg: StyleConfig) {
    *PATH_DISPLAY_CONFIG.write().unwrap() = style_cfg
}
pub fn global_ui_mut() -> RwLockWriteGuard<'static, StyleConfig> {
    PATH_DISPLAY_CONFIG.write().unwrap()
}

pub fn global_ui() -> RwLockReadGuard<'static, StyleConfig> {
    PATH_DISPLAY_CONFIG.read().unwrap()
}
