use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::ui::styles_config::{PathDisplayConfig, StyleConfig};

static PATH_DISPLAY_CONFIG: RwLock<PathDisplayConfig> = RwLock::new(PathDisplayConfig::DEFAULT);
pub fn global_ui_init(style_cfg: StyleConfig) {
    *PATH_DISPLAY_CONFIG.write().unwrap() = style_cfg.path
}
pub fn global_ui_mut() -> RwLockWriteGuard<'static, PathDisplayConfig> {
    PATH_DISPLAY_CONFIG.write().unwrap()
}

pub fn global_ui() -> RwLockReadGuard<'static, PathDisplayConfig> {
    PATH_DISPLAY_CONFIG.read().unwrap()
}
