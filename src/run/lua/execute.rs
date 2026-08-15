//! Isolated VM per execution.
//!
//! Menu-action commands (direct and queued) run on a fresh [`Lua`] per
//! call: concurrent executions (ExecuteAsync tasks, detached ExecuteSilent
//! threads, queued items) never share or serialize on VM state, and script
//! globals cannot leak into later runs.

use std::{cell::Cell, ptr::NonNull, sync::atomic::AtomicU8};

use mlua::{Lua, MultiValue, Value};

use crate::abspath::AbsPath;

// The progress cell of the queue item whose command is running on this
// thread, if any. The target is set only while a queue item's lua command
// runs (see [`crate::run::queue::QueueItem`](crate::run::queue::execute));
// lua code from any other context (pane transform scripts, Execute/
// ExecuteSilent menu commands) has no target and `set_progress` is a
// silent no-op there.
thread_local! {
    static PROGRESS_TARGET: Cell<Option<NonNull<AtomicU8>>> = const { Cell::new(None) };
}

/// Run `source` with the `(paths, dst)` contract: `paths` is always a table
/// of path strings (a one-element table for a single path) and `dst` is
/// passed verbatim. When `nav_cwd` is given it is passed as the optional
/// third argument — `(paths, dst)` without it, `(paths, dst, nav_cwd)` with
/// it. When `progress` is given, `set_progress` writes to it for the
/// duration of the call; the target is cleared afterwards and is a silent
/// no-op for callers that pass `None`.
///
/// A fresh [`Lua`] is created per call: compile errors surface here, the
/// VM is dropped with the call, and `os.exit` is overridden so a script
/// stops itself with a runtime error carrying the exit code instead of
/// terminating the host process.
pub fn execute(
    source: &str,
    paths: &[AbsPath],
    dst: &str,
    nav_cwd: Option<&AbsPath>,
    progress: Option<&AtomicU8>,
) -> Result<MultiValue, String> {
    let lua = Lua::new();
    register_progress_global(&lua)?;
    override_os_exit(&lua)?;
    let f = lua
        .load(source)
        .into_function()
        .map_err(|e| e.to_string())?;

    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (i, p) in paths.iter().enumerate() {
        table
            .raw_seti(i + 1, p.to_string_lossy().into_owned())
            .map_err(|e| e.to_string())?;
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
    res.map_err(|e| e.to_string())
}

/// Compile `source` against a scratch VM without running it; used by
/// `fs :tool check`.
pub fn check_compiles(source: &str) -> Result<(), String> {
    let lua = Lua::new();
    lua.load(source)
        .into_function()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Register the `set_progress(v)` global on `lua`. `v` is on the internal
/// 0-255 scale and is written to the executing queue item's progress cell;
/// see [`PROGRESS_TARGET`] for when a target exists.
fn register_progress_global(lua: &Lua) -> Result<(), String> {
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
        .map_err(|e| e.to_string())?;
    lua.globals()
        .set("set_progress", f)
        .map_err(|e| e.to_string())
}

/// Override `os.exit` so a script's `os.exit(code)` stops only the script
/// (a runtime error carrying the code) instead of terminating the host
/// process (lua 5.4's `os.exit` calls `exit(3)`).
fn override_os_exit(lua: &Lua) -> Result<(), String> {
    let os_table = lua
        .globals()
        .get::<mlua::Table>("os")
        .map_err(|e| e.to_string())?;
    let f = lua
        .create_function(|_, status: Option<Value>| -> mlua::Result<()> {
            let code = match status {
                Some(Value::Boolean(true)) | None => 0,
                Some(Value::Boolean(false)) => 1,
                Some(Value::Integer(n)) => n,
                Some(Value::Number(n)) => n as i64,
                Some(other) => {
                    return Err(mlua::Error::RuntimeError(format!(
                        "os.exit: unsupported argument {other:?}"
                    )))
                }
            };
            Err(mlua::Error::RuntimeError(format!("os.exit({code})")))
        })
        .map_err(|e| e.to_string())?;
    os_table.set("exit", f).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abspath::AbsPath;

    fn path(s: &str) -> AbsPath {
        AbsPath::new(std::path::PathBuf::from(s))
    }

    #[test]
    fn test_execute_git_diff() {
        let cmd = r#"os.execute('cd "' .. (...)[1] .. '" 2>/dev/null || cd "$(dirname "' .. (...)[1] .. '")"; git diff -- "' .. (...)[1] .. '"')"#;
        let res = execute(cmd, &[path("/tmp")], "", None, None);
        assert!(res.is_ok(), "execute failed: {res:?}");
    }

    #[test]
    fn test_execute_set_progress() {
        let cmd = r#"for i = 1, 2 do set_progress(math.floor(i / 2 * 255)) end"#;
        let progress = AtomicU8::new(0);
        let paths = [path("/tmp/a"), path("/tmp/b")];
        let res = execute(cmd, &paths, "", None, Some(&progress));
        assert!(res.is_ok(), "execute failed: {res:?}");
        assert_eq!(progress.load(std::sync::atomic::Ordering::Relaxed), 255);

        // without a progress target set_progress is a silent no-op
        let res = execute(cmd, &paths, "", None, None);
        assert!(res.is_ok(), "execute failed: {res:?}");
    }

    #[test]
    fn test_os_exit_stops_only_the_script() {
        // the test process survives: os.exit is overridden per-VM
        let res = execute("os.exit(3)", &[], "", None, None);
        assert!(res.is_err(), "os.exit(3) should error: {res:?}");
        assert!(res.unwrap_err().contains("os.exit(3)"));

        let res = execute("os.exit()", &[], "", None, None);
        assert!(res.is_err(), "os.exit() should error");
        assert!(res.unwrap_err().contains("os.exit(0)"));

        // a script after os.exit never runs
        let res = execute("os.exit(1) error('unreachable')", &[], "", None, None);
        assert!(res.unwrap_err().contains("os.exit(1)"));
    }

    #[test]
    fn test_execution_isolation() {
        // globals set by one execution are invisible to the next
        let res = execute("leaked = 'x'", &[], "", None, None);
        assert!(res.is_ok(), "execute failed: {res:?}");
        let res = execute(
            "if leaked ~= nil then error('global leaked between executions') end",
            &[],
            "",
            None,
            None,
        );
        assert!(res.is_ok(), "execute failed: {res:?}");
    }

    #[test]
    fn test_execute_at_file_direct() {
        // the direct path must resolve @file scripts (load_script) before
        // compiling — the original direct-run bug compiled the raw "@path"
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("hello.lua");
        let marker = dir.path().join("marker");
        std::fs::write(
            &script,
            format!("local f = assert(io.open({marker:?}, 'w')) f:write('ok') f:close()"),
        )
        .unwrap();
        let cmd = format!("@{}", script.display());
        let src = crate::run::lua::load_script(&cmd, None).expect("script should load");
        let res = execute(&src, &[path("/tmp")], "", None, None);
        assert!(res.is_ok(), "execute failed: {res:?}");
        assert_eq!(
            std::fs::read_to_string(&marker).unwrap(),
            "ok",
            "@file script should have run"
        );
    }

    #[test]
    fn test_load_script_base() {
        // relative @paths resolve against the base; base-less keeps cwd
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("actions");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("run.lua"), "return 1").unwrap();

        let src = crate::run::lua::load_script("@run.lua", Some(&base)).expect("should load");
        assert!(src.contains("return 1"));

        // absolute paths ignore the base
        let src = crate::run::lua::load_script(
            &format!("@{}", base.join("run.lua").display()),
            Some(&base),
        )
        .expect("should load");
        assert!(src.contains("return 1"));

        // a missing file is None
        assert!(crate::run::lua::load_script("@nope.lua", Some(&base)).is_none());
    }
}
