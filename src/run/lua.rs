use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};

use cba::bath::PathExt;
use mlua::Lua;

use crate::abspath::AbsPath;

pub type LuaFn = mlua::Function;

static VM: OnceLock<&'static Lua> = OnceLock::new();
// a single Lua state is not internally thread-safe; populate tasks may overlap (reload)
static LUA_LOCK: Mutex<()> = Mutex::new(());

fn lua_vm() -> &'static Lua {
    VM.get_or_init(|| Box::leak(Box::new(Lua::new())))
}

/// Compile `script` against the process-wide VM (leaked, thread-safe).
pub fn compile_lua(
    script: &str,
) -> Result<LuaFn, String> {
    let _g = LUA_LOCK.lock().unwrap();
    lua_vm()
        .load(script)
        .eval::<LuaFn>()
        .map_err(|e| e.to_string())
}

/// Call a precompiled function with the item's path string; result must be a string.
/// Never panics on script failure — returns the empty string instead.
pub fn call_lua(
    f: &LuaFn,
    path: &AbsPath,
) -> String {
    let _g = LUA_LOCK.lock().unwrap();
    f.call::<String>(path.to_string_lossy().as_ref())
        .unwrap_or_default()
}

/// Runs the pane's path transform on the raw path string; on error/absence
/// returns the input unchanged. Applied before `PathItem` construction so
/// rendering, sorting, and output all see the mapped path.
pub fn transform_path(
    script: Option<&LuaFn>,
    raw: &str,
    cwd: &Path,
) -> String {
    match script {
        Some(f) => {
            let p = AbsPath::new_unchecked(raw.abs(cwd));
            call_lua(f, &p)
        }
        None => raw.to_string(),
    }
}

/// `@path` → file contents; anything else → the string itself.
pub fn load_script(s: &str) -> Option<String> {
    match s.strip_prefix('@') {
        Some(mut p) => {
            let expanded;
            if let Some(rest) = p.strip_prefix("~/") {
                if let Some(home) = dirs::home_dir() {
                    expanded = home.join(rest).to_string_lossy().into_owned();
                    p = &expanded;
                }
            }
            match std::fs::read_to_string(p) {
                Ok(s) => Some(s),
                Err(e) => {
                    log::error!("Failed to read lua script @{p}: {e}");
                    None
                }
            }
        }
        None => Some(s.to_string()),
    }
}

/// Load (2.2 `@file` syntax) and compile a script; bad scripts are dropped with an error log.
pub fn compile_script(
    name: &str,
    script: Option<String>,
) -> Option<LuaFn> {
    script.and_then(|s| load_script(&s)).and_then(|s| match compile_lua(&s) {
        Ok(f) => Some(f),
        Err(e) => {
            log::error!("Failed to compile {name} lua script: {e}");
            None
        }
    })
}
