//! Isolated VM per execution.
//!
//! Menu-action commands (direct and queued) run on a fresh [`Lua`] per
//! call: concurrent executions (ExecuteAsync tasks, detached ExecuteSilent
//! threads, queued items) never share or serialize on VM state, and script
//! globals cannot leak into later runs.

use std::{cell::Cell, ptr::NonNull, sync::atomic::AtomicU8};

use mlua::{Lua, MultiValue, Value};

use matchmaker::nucleo::Span;

use crate::{
    abspath::AbsPath,
    run::state::{TOAST, ToastStyle},
};

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
    register_toast_globals(&lua)?;
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
    let globals = lua.globals();
    globals
        .set("paths", table.clone())
        .map_err(|e| e.to_string())?;
    globals
        .set("dst", dst.to_string())
        .map_err(|e| e.to_string())?;
    if let Some(cwd) = nav_cwd {
        globals
            .set("nav_cwd", cwd.to_string_lossy().into_owned())
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
    register_progress_global(&lua)?;
    register_toast_globals(&lua)?;
    lua.load(source)
        .into_function()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Parse a style string into [`ToastStyle`].
fn parse_toast_style(s: Option<&str>) -> ToastStyle {
    match s.map(|x| x.trim().to_ascii_lowercase()).as_deref() {
        Some("info") => ToastStyle::Info,
        Some("success") => ToastStyle::Success,
        Some("warning" | "warn") => ToastStyle::Warning,
        Some("error" | "err") => ToastStyle::Error,
        _ => ToastStyle::Normal,
    }
}

/// Register `toast(style, msg)` and `toast_push(style, prefix, item)` globals on `lua`.
fn register_toast_globals(lua: &Lua) -> Result<(), String> {
    let toast_fn = lua
        .create_function(|_, (style, msg): (Option<String>, String)| {
            let toast_style = parse_toast_style(style.as_deref());
            TOAST::notice(toast_style, msg);
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    let toast_push_fn = lua
        .create_function(
            |_, (style, prefix, item): (Option<String>, String, String)| {
                let toast_style = parse_toast_style(style.as_deref());
                TOAST::push(toast_style, prefix, vec![Span::raw(item)]);
                Ok(())
            },
        )
        .map_err(|e| e.to_string())?;

    let globals = lua.globals();
    globals.set("toast", toast_fn).map_err(|e| e.to_string())?;
    globals
        .set("toast_push", toast_push_fn)
        .map_err(|e| e.to_string())?;
    Ok(())
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
                    )));
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
    use crate::run::state::GLOBAL;

    fn path(s: &str) -> AbsPath {
        AbsPath::new(std::path::PathBuf::from(s))
    }

    #[test]
    fn test_execute_chmod_script() {
        use std::os::unix::fs::PermissionsExt;

        GLOBAL::init_test_senders();
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("script.sh");
        std::fs::write(&file_path, "#!/bin/sh\necho hello\n").unwrap();

        std::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            std::fs::metadata(&file_path).unwrap().permissions().mode() & 0o111,
            0
        );

        let abs = path(file_path.to_str().unwrap());
        let cmd = r#"
            local function shq(s) return "'" .. tostring(s):gsub("'", "'\\''") .. "'" end
            local all_exec = true
            for _, p in ipairs(paths) do
              if not os.execute("test -x " .. shq(p)) then
                all_exec = false
                break
              end
            end
            local mode = all_exec and "-x" or "+x"
            local dir = paths[1]:match("^(.*)/") or "."
            local names = {}
            for _, p in ipairs(paths) do
              names[#names + 1] = shq("./" .. (p:match("([^/]+)/?$") or "."))
            end
            local ok, how, code = os.execute(
              "cd " .. shq(dir) .. " && chmod " .. mode .. " " .. table.concat(names, " "))
            if not ok then error("chmod " .. mode .. " failed: " .. tostring(code)) end

            local prefix = "set " .. mode .. ": "
            local item = #paths == 1 and (paths[1]:match("([^/]+)/?$") or paths[1]) or (#paths .. " items")
            toast_push("success", prefix, item)
        "#;
        // 1st run: sets +x
        let res = execute(cmd, &[abs.clone()], "", None, None);
        assert!(res.is_ok(), "execute +x failed: {res:?}");
        assert_ne!(
            std::fs::metadata(&file_path).unwrap().permissions().mode() & 0o111,
            0,
            "file should be executable after first toggle"
        );

        // 2nd run: toggles back to -x
        let res = execute(cmd, &[abs], "", None, None);
        assert!(res.is_ok(), "execute -x failed: {res:?}");
        assert_eq!(
            std::fs::metadata(&file_path).unwrap().permissions().mode() & 0o111,
            0,
            "file should be non-executable after second toggle"
        );
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
        let src = crate::lua::load_script(&cmd, None).expect("script should load");
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

        let src = crate::lua::load_script("@run.lua", Some(&base)).expect("should load");
        assert!(src.contains("return 1"));

        // absolute paths ignore the base
        let src =
            crate::lua::load_script(&format!("@{}", base.join("run.lua").display()), Some(&base))
                .expect("should load");
        assert!(src.contains("return 1"));

        // a missing file is None
        assert!(crate::lua::load_script("@nope.lua", Some(&base)).is_none());
    }

    #[test]
    fn test_lua_toast_functions() {
        GLOBAL::init_test_senders();
        let cmd = r#"
            toast("info", "Information message")
            toast_push("success", "Processed: ", "file.txt")
            toast_push("warning", "Skipped: ", "file2.txt")
            toast("error", "Error message")
            toast(nil, "Default normal message")
        "#;
        let res = execute(cmd, &[], "", None, None);
        assert!(res.is_ok(), "execute failed: {res:?}");
    }
}
