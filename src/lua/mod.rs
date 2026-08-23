//! Lua scripting support.
//!
//! Two execution models:
//!
//! - [`execute::execute`] runs a script on a **fresh VM per execution** —
//!   used for menu-action commands (direct and queued). Concurrent
//!   executions never share or serialize on VM state, and scripts cannot
//!   leak globals into later runs.
//! - [`inject`] provides a **reusable leaked VM** for pane `--transform`
//!   scripts, whose per-row invocation must be cheap.
//!
//! [`load_script`] resolves the `@file` syntax shared by both models.

mod execute;
mod inject;

pub use execute::{check_compiles, execute};
pub use inject::{LuaFn, call_transform, compile_script};

use std::path::Path;

/// `@path` → file contents; anything else → the string itself. A relative
/// `@path` resolves against `base` when given (menu actions use the actions
/// folder); the base-less form keeps cwd-relative resolution (`--transform`
/// panes, lessfilter presets). `~/` is expanded to the home directory.
pub fn load_script(
    s: &str,
    base: Option<&Path>,
) -> Option<String> {
    match s.strip_prefix('@') {
        Some(mut p) => {
            let expanded;
            if let Some(rest) = p.strip_prefix("~/") {
                if let Some(home) = dirs::home_dir() {
                    expanded = home.join(rest).to_string_lossy().into_owned();
                    p = &expanded;
                }
            }
            let path = {
                let p_path = Path::new(p);
                if p_path.is_absolute() {
                    p_path.to_path_buf()
                } else if let Some(base) = base {
                    base.join(p)
                } else {
                    p_path.to_path_buf()
                }
            };
            match std::fs::read_to_string(&path) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::error!("Failed to read lua script @{}: {e}", path.display());
                    None
                }
            }
        }
        None => Some(s.to_string()),
    }
}
