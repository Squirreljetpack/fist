//! Reusable VM for pane `--transform` injections.
//!
//! Transforms are compiled once per pane and called per row; a fresh VM per
//! row would be far too slow, so this module keeps the process-wide leaked
//! VM guarded by a lock (population tasks may overlap across reloads).
//! Menu-action executions use [`super::execute::execute`] instead and never
//! touch this VM.

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
fn compile_lua(script: &str) -> Result<LuaFn, String> {
    let _g = LUA_LOCK.lock().unwrap();
    lua_vm()
        .load(script)
        .into_function()
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

/// Load (2.2 `@file` syntax) and compile a script; bad scripts are dropped
/// with an error log. `@file` paths resolve cwd-relative (`None` base).
pub fn compile_script(
    name: &str,
    script: Option<String>,
) -> Option<LuaFn> {
    script
        .and_then(|s| super::load_script(&s, None))
        .and_then(|s| match compile_lua(&s) {
            Ok(f) => Some(f),
            Err(e) => {
                log::error!("Failed to compile {name} lua script: {e}");
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_contract() {
        // (path, tail) -> (path, display, tail); missing outputs stay None
        let f = compile_script(
            "--transform",
            Some(r#"local p, t = ...; return p, t .. "X", t"#.into()),
        )
        .unwrap();
        let p = AbsPath::new(std::path::PathBuf::from("/tmp/a.txt"));
        let (path, display, tail) = call_transform(&f, &p, "tail").unwrap();
        assert_eq!(path.as_deref(), Some("/tmp/a.txt"));
        assert_eq!(display.as_deref(), Some("tailX"));
        assert_eq!(tail.as_deref(), Some("tail"));

        // a script returning nothing yields all-None outputs
        let f = compile_script("--transform", Some("return".into())).unwrap();
        let (path, display, tail) = call_transform(&f, &p, "tail").unwrap();
        assert_eq!(path, None);
        assert_eq!(display, None);
        assert_eq!(tail, None);
    }

    #[test]
    fn test_transform_unaffected_by_execution_globals() {
        // a global set by an isolated menu-action execution is invisible to
        // the shared transform VM
        let res = crate::lua::execute("leaked = 'x'", &[], "", None, None);
        assert!(res.is_ok());

        let f = compile_script(
            "--transform",
            Some(
                r#"if leaked ~= nil then error('execution global leaked into transform') end
return ..."#
                    .into(),
            ),
        )
        .unwrap();
        let p = AbsPath::new(std::path::PathBuf::from("/tmp/a.txt"));
        let (path, ..) = call_transform(&f, &p, "").unwrap();
        assert_eq!(path.as_deref(), Some("/tmp/a.txt"));
    }
}
