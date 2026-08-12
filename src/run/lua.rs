use std::sync::{Mutex, OnceLock};

use mlua::{Lua, MultiValue};

use crate::abspath::AbsPath;

pub type LuaFn = mlua::Function;

static VM: OnceLock<&'static Lua> = OnceLock::new();
// a single Lua state is not internally thread-safe; populate tasks may overlap (reload)
static LUA_LOCK: Mutex<()> = Mutex::new(());

fn lua_vm() -> &'static Lua {
    VM.get_or_init(|| Box::leak(Box::new(Lua::new())))
}

/// Compile `script` against the process-wide VM (leaked, thread-safe).
pub fn compile_lua(script: &str) -> Result<LuaFn, String> {
    let _g = LUA_LOCK.lock().unwrap();
    lua_vm()
        .load(script)
        .eval::<LuaFn>()
        .map_err(|e| e.to_string())
}

/// Call the pane's transform: `(path, tail) -> (path, display, tail)`.
/// Each return value is optional: a missing or non-string output yields
/// `None` — a `None` path omits the entry from the listing, `None`
/// display/tail keep the current values. Script errors are propagated.
/// Never panics.
pub fn call_transform(
    f: &LuaFn,
    path: &AbsPath,
    tail: &str,
) -> anyhow::Result<(Option<String>, Option<String>, Option<String>)> {
    let _g = LUA_LOCK.lock().unwrap();
    let path = path.to_string_lossy();
    let vals = f.call::<MultiValue>((path.as_ref(), tail))?;
    let mut it = vals.into_iter();
    let path = it
        .next()
        .and_then(|v| v.as_str().as_deref().map(str::to_owned));
    let display = it
        .next()
        .and_then(|v| v.as_str().as_deref().map(str::to_owned));
    let tail = it
        .next()
        .and_then(|v| v.as_str().as_deref().map(str::to_owned));
    Ok((path, display, tail))
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
    script
        .and_then(|s| load_script(&s))
        .and_then(|s| match compile_lua(&s) {
            Ok(f) => Some(f),
            Err(e) => {
                log::error!("Failed to compile {name} lua script: {e}");
                None
            }
        })
}
