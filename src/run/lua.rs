use std::{
    cell::Cell,
    ptr::NonNull,
    sync::atomic::AtomicU8,
    sync::{Mutex, OnceLock},
};

use mlua::{Lua, MultiValue};

use crate::abspath::AbsPath;

pub type LuaFn = mlua::Function;

static VM: OnceLock<&'static Lua> = OnceLock::new();
// a single Lua state is not internally thread-safe; populate tasks may overlap (reload)
static LUA_LOCK: Mutex<()> = Mutex::new(());

// The progress cell of the queue item whose command is running on this
// thread, if any. The target is set only while a queue item's lua command
// runs (see [`QueueItem::execute`](crate::run::queue::QueueItem)); lua
// code from any other context (pane transform scripts, Execute/
// ExecuteSilent menu commands) has no target and `set_progress` is a
// silent no-op there.
thread_local! {
    static PROGRESS_TARGET: Cell<Option<NonNull<AtomicU8>>> = const { Cell::new(None) };
}

fn lua_vm() -> &'static Lua {
    VM.get_or_init(|| {
        let lua = Box::leak(Box::new(Lua::new()));
        register_progress_global(lua);
        lua
    })
}

/// Register the `set_progress(v)` global. `v` is on the internal 0-255
/// scale and is written to the executing queue item's progress cell; see
/// [`PROGRESS_TARGET`] for when a target exists.
fn register_progress_global(lua: &Lua) {
    let f = lua
        .create_function(|_, v: u8| {
            PROGRESS_TARGET.with(|t| {
                if let Some(target) = t.get() {
                    // SAFETY: the pointer is valid only while a queue item's
                    // command runs; the item outlives the call.
                    unsafe { target.as_ref() }.store(v, std::sync::atomic::Ordering::Relaxed);
                }
            });
            Ok(())
        })
        .expect("failed to create set_progress function");
    lua.globals()
        .set("set_progress", f)
        .expect("failed to register set_progress");
}

/// Compile `script` against the process-wide VM (leaked, thread-safe).
pub fn compile_lua(script: &str) -> Result<LuaFn, String> {
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

/// Call `f` with the `(paths, dst)` contract: `paths` is always a table of
/// path strings (a one-element table for a single path) and `dst` is passed
/// verbatim. When `nav_cwd` is given it is passed as the optional third
/// argument — `(paths, dst)` without it, `(paths, dst, nav_cwd)` with it.
/// When `progress` is given, `set_progress` writes to it for the duration of
/// the call; the target is cleared afterwards and is a silent no-op for
/// callers that pass `None`.
pub fn call_with_paths(
    f: &LuaFn,
    paths: &[AbsPath],
    dst: &str,
    nav_cwd: Option<&AbsPath>,
    progress: Option<&AtomicU8>,
) -> Result<MultiValue, mlua::Error> {
    let _g = LUA_LOCK.lock().unwrap();
    let table = lua_vm().create_table()?;
    for (i, p) in paths.iter().enumerate() {
        table.raw_seti(i + 1, p.to_string_lossy().into_owned())?;
    }
    if let Some(progress) = progress {
        PROGRESS_TARGET.with(|t| t.set(Some(NonNull::from(progress))));
    }
    let res = match nav_cwd {
        Some(cwd) => {
            f.call::<MultiValue>((table, dst.to_string(), cwd.to_string_lossy().into_owned()))
        }
        None => f.call::<MultiValue>((table, dst.to_string())),
    };
    PROGRESS_TARGET.with(|t| t.set(None));
    res
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_git_diff() {
        let cmd = r#"os.execute('cd "' .. (...)[1] .. '" 2>/dev/null || cd "$(dirname "' .. (...)[1] .. '")"; git diff -- "' .. (...)[1] .. '"')"#;
        let res = compile_lua(cmd);
        assert!(res.is_ok(), "compile_lua failed: {:?}", res);

        let f = res.unwrap();
        let paths = vec![AbsPath::new(std::path::PathBuf::from("/tmp"))];
        let call_res = call_with_paths(&f, &paths, "", None, None);
        assert!(call_res.is_ok(), "call_with_paths failed: {:?}", call_res);
    }

    #[test]
    fn test_compile_stash_demo() {
        let cmd = r#"for i = 1, 2 do set_progress(math.floor(i / 2 * 255)) end"#;
        let res = compile_lua(cmd);
        assert!(res.is_ok(), "compile_lua failed: {:?}", res);

        let f = res.unwrap();
        let progress = std::sync::atomic::AtomicU8::new(0);
        let paths = vec![
            AbsPath::new(std::path::PathBuf::from("/tmp/a")),
            AbsPath::new(std::path::PathBuf::from("/tmp/b")),
        ];
        let call_res = call_with_paths(&f, &paths, "", None, Some(&progress));
        assert!(call_res.is_ok(), "call_with_paths failed: {:?}", call_res);
        assert_eq!(progress.load(std::sync::atomic::Ordering::Relaxed), 255);
    }
}
