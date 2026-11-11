//! Isolated VM per execution.
//!
//! Menu-action commands (direct and queued) run on a fresh [`Lua`] per
//! call: concurrent executions (ExecuteAsync tasks, detached ExecuteSilent
//! threads, queued items) never share or serialize on VM state, and script
//! globals cannot leak into later runs.

use std::{
    cell::Cell,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use mlua::{Lua, MultiValue, Value};

use matchmaker::nucleo::Span;

use crate::{
    abspath::AbsPath,
    run::state::{TOAST, ToastStyle},
};

// The progress cell of the queue item whose command is running on this
// thread, if any. The target is set only while a queue item's lua command
// runs (see [`crate::run::queue::QueueItem`](crate::run::queue::execute));
// `set_progress` is not defined at all in any other context (pane transform
// scripts, Exec/ExecSilent menu commands) since it could do nothing.
thread_local! {
    static PROGRESS_TARGET: Cell<Option<NonNull<AtomicU8>>> = const { Cell::new(None) };
}

/// Run `source` with the `(paths, to)` contract: `paths` is always a table
/// of path strings (a one-element table for a single path) and `to` is
/// passed verbatim. When `nav_cwd` is given it is passed as the optional
/// third argument — `(paths, to)` without it, `(paths, to, nav_cwd)` with
/// it. When `progress` is given, `set_progress` is defined and writes to it for
/// the duration of the call; the target is cleared afterwards. Callers that
/// pass `None` (menu runs) do not get a `set_progress` global at all, which
/// keeps scripts from believing they report progress they cannot report.
///
/// A fresh [`Lua`] is created per call: compile errors surface here, the
/// VM is dropped with the call, and `os.exit` is overridden so a script
/// stops itself with a runtime error carrying the exit code instead of
/// terminating the host process.
pub fn execute(
    source: &str,
    paths: &[AbsPath],
    to: &str,
    nav_cwd: Option<&AbsPath>,
    progress: Option<&AtomicU8>,
    allow_input: bool,
) -> Result<MultiValue, String> {
    let lua = Lua::new();
    if progress.is_some() {
        register_progress_global(&lua)?;
    }
    register_toast_globals(&lua)?;
    let is_interactive = allow_input && progress.is_none();
    super::modules::register_modules(&lua, is_interactive).map_err(|e| e.to_string())?;
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
        .set("to", to.to_string())
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
            f.call::<MultiValue>((table, to.to_string(), cwd.to_string_lossy().into_owned()))
        }
        None => f.call::<MultiValue>((table, to.to_string())),
    };
    PROGRESS_TARGET.with(|t| t.set(None));
    res.map_err(|e| e.to_string())
}

/// Compile `source` against a scratch VM without running it; used by
/// `fs :tool check`.
pub fn check_compiles(source: &str) -> Result<(), String> {
    let lua = Lua::new();
    register_toast_globals(&lua)?;
    super::modules::register_modules(&lua, true).map_err(|e| e.to_string())?;
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

/// Print toasts to stderr instead of queueing them for the UI.
///
/// A command-line run has no toast overlay, so [`crate::cli::action`] turns
/// this on to keep an action's messages visible.
pub fn set_toasts_to_stderr(enable: bool) {
    TOASTS_TO_STDERR.store(enable, Ordering::Relaxed);
}

fn toasts_to_stderr() -> bool {
    TOASTS_TO_STDERR.load(Ordering::Relaxed)
}

static TOASTS_TO_STDERR: AtomicBool = AtomicBool::new(false);

/// Register `toast(style, msg)` and `toast_push(style, prefix, item)` globals on `lua`.
fn register_toast_globals(lua: &Lua) -> Result<(), String> {
    let toast_fn = lua
        .create_function(|_, (style, msg): (Option<String>, String)| {
            let toast_style = parse_toast_style(style.as_deref());
            if toasts_to_stderr() {
                eprintln!("{toast_style}: {msg}");
            } else {
                TOAST::notice(toast_style, msg);
            }
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    let toast_push_fn = lua
        .create_function(
            |_, (style, prefix, item): (Option<String>, String, String)| {
                let toast_style = parse_toast_style(style.as_deref());
                if toasts_to_stderr() {
                    eprintln!("{toast_style}: {prefix}{item}");
                } else {
                    TOAST::push(toast_style, prefix, vec![Span::raw(item)]);
                }
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

/// Register the `set_progress(v)` global on `lua`, for the executions that
/// have a progress cell to write to. `v` is on the internal 0-255 scale.
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
            local all_exec = true
            for _, p in ipairs(paths) do
              local st = fs.stat(p)
              if not (st and st.executable) then
                all_exec = false
                break
              end
            end
            local mode = all_exec and "-x" or "+x"
            local args = { mode, "--" }
            for _, p in ipairs(paths) do
              args[#args + 1] = p
            end
            local res = proc.run("chmod", args)
            if not res.ok then error("chmod " .. mode .. " failed: " .. tostring(res.code)) end

            local prefix = "set " .. mode .. ": "
            local item = #paths == 1 and path.base(paths[1]) or (#paths .. " items")
            toast_push("success", prefix, item)
        "#;
        // 1st run: sets +x
        let res = execute(cmd, std::slice::from_ref(&abs), "", None, None, false);
        assert!(res.is_ok(), "execute +x failed: {res:?}");
        assert_ne!(
            std::fs::metadata(&file_path).unwrap().permissions().mode() & 0o111,
            0,
            "file should be executable after first toggle"
        );

        // 2nd run: toggles back to -x
        let res = execute(cmd, &[abs], "", None, None, false);
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
        let res = execute(cmd, &paths, "", None, Some(&progress), false);
        assert!(res.is_ok(), "execute failed: {res:?}");
        assert_eq!(progress.load(std::sync::atomic::Ordering::Relaxed), 255);

        // without a progress target there is no set_progress at all
        let res = execute("set_progress(1)", &paths, "", None, None, false);
        assert!(
            res.is_err(),
            "set_progress must be undefined outside queue runs: {res:?}"
        );
        let res = execute("assert(set_progress == nil)", &paths, "", None, None, false);
        assert!(res.is_ok(), "execute failed: {res:?}");
    }

    #[test]
    fn test_os_exit_stops_only_the_script() {
        // the test process survives: os.exit is overridden per-VM
        let res = execute("os.exit(3)", &[], "", None, None, false);
        assert!(res.is_err(), "os.exit(3) should error: {res:?}");
        assert!(res.unwrap_err().contains("os.exit(3)"));

        let res = execute("os.exit()", &[], "", None, None, false);
        assert!(res.is_err(), "os.exit() should error");
        assert!(res.unwrap_err().contains("os.exit(0)"));

        // a script after os.exit never runs
        let res = execute(
            "os.exit(1) error('unreachable')",
            &[],
            "",
            None,
            None,
            false,
        );
        assert!(res.unwrap_err().contains("os.exit(1)"));
    }

    #[test]
    fn test_execution_isolation() {
        // globals set by one execution are invisible to the next
        let res = execute("leaked = 'x'", &[], "", None, None, false);
        assert!(res.is_ok(), "execute failed: {res:?}");
        let res = execute(
            "if leaked ~= nil then error('global leaked between executions') end",
            &[],
            "",
            None,
            None,
            false,
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
        let res = execute(&src, &[path("/tmp")], "", None, None, false);
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
        let res = execute(cmd, &[], "", None, None, false);
        assert!(res.is_ok(), "execute failed: {res:?}");
    }

    #[test]
    fn test_lua_lunacolors_bundled() {
        let cmd = r#"
            local req_lc = require("lunacolors")
            assert(type(req_lc) == "table", "require('lunacolors') should return a table")
            assert(type(lunacolors) == "table", "global lunacolors should be available")
            assert(type(Lunacolors) == "table", "global Lunacolors should be available")

            local b = lunacolors.bold("test")
            assert(b == "\27[1mtest\27[22m", "bold formatting mismatch: " .. b)

            local u = lunacolors.underline("test")
            assert(u == "\27[4mtest\27[24m", "underline formatting mismatch: " .. u)

            local bu = lunacolors.bold(lunacolors.underline("test"))
            assert(bu == "\27[1m\27[4mtest\27[24m\27[22m", "chained bold+underline mismatch: " .. bu)

            local fmt = lunacolors.format("{bold}{underline}test")
            assert(fmt == "\27[1m\27[4mtest\27[0m", "format mismatch: " .. fmt)
        "#;
        let res = execute(cmd, &[], "", None, None, false);
        assert!(res.is_ok(), "lunacolors test failed: {res:?}");
    }

    #[test]
    fn test_lua_modules_util_and_input() {
        GLOBAL::init_test_senders();
        let cmd = r#"
            local u = require("util")
            assert(type(u) == "table", "util module should load")
            assert(type(util) == "table", "global util should exist")

            -- util.sleep
            util.sleep(10)

            -- util.uuid
            local id = util.uuid()
            assert(#id == 36, "uuid must be 36 characters")
            assert(id:match("^%x%x%x%x%x%x%x%x%-%x%x%x%x%-4%x%x%x%-[89ab]%x%x%x%-%x%x%x%x%x%x%x%x%x%x%x%x$"), "valid uuid v4 format")

            -- util.timestamp
            local ts = util.timestamp()
            assert(#ts > 10, "timestamp non-empty")

            -- util.json_encode / decode
            local data = { name = "test", count = 42, active = true, tags = {"a", "b"} }
            local json_str = util.json_encode(data)
            local decoded = util.json_decode(json_str)
            assert(decoded.name == "test", "json decode name")
            assert(decoded.count == 42, "json decode count")
            assert(decoded.active == true, "json decode active")
            assert(decoded.tags[1] == "a" and decoded.tags[2] == "b", "json decode array")

            -- util.toml_encode / decode
            local toml_str = util.toml_encode({ title = "TOML Test", port = 8080 })
            local toml_decoded = util.toml_decode(toml_str)
            assert(toml_decoded.title == "TOML Test", "toml decode title")
            assert(toml_decoded.port == 8080, "toml decode port")

            -- util shq
            assert(util.shq("hello world") == "'hello world'", "shq")

            -- path module
            assert(type(path) == "table", "global path table")
            assert(path.parent("/a/b/c.txt") == "/a/b", "path.parent")
            assert(path.parent("/root_file.txt") == "/", "path.parent of root")
            assert(path.base("/a/b/c.txt") == "c.txt", "path.base")
            assert(path.stem("/a/b/c.txt") == "c", "path.stem")
            assert(path.ext("/a/b/c.txt") == "txt", "path.ext")
            assert(path.join("a", "b", "c.txt") == "a/b/c.txt", "path.join")
            assert(path.exists("/") == true, "path.exists(/)")
            assert(path.lexists("/") == true, "path.lexists(/)")
            assert(path.rel("/a/b/c", "/a") == "b/c", "path.rel")
            assert(path.rel("/a/b", "/a/b") == "", "path.rel identical")
            assert(path.rel("/a/b", "c") == "/a/b", "path.rel absolute target")
            assert(path.rel("a", "/b") == nil, "path.rel absolute base")
            assert(path.mime("test.png") == "image/png", "path.mime")
            assert(util.path == nil, "util.path should not exist")

            -- proc module
            assert(type(proc) == "table", "proc table")
            local echo_res = proc.run("echo", {"hello"})
            assert(echo_res.ok == true, "proc.run echo")
            assert(echo_res.stdout:find("hello") ~= nil, "proc.run stdout")

            -- fs module
            assert(type(fs) == "table", "fs table")
            local tmp = fs.tempfile("fist_unit_")
            assert(fs.write(tmp, "hello world") == true, "fs.write")
            assert(fs.read(tmp) == "hello world", "fs.read")
            assert(fs.stat(tmp).size == 11, "fs.stat size")
            assert(fs.remove(tmp) == true, "fs.remove")

            -- interactive-only modules
            assert(type(proc.editor) == "function", "proc.editor should exist in interactive run")
            assert(type(input) == "table", "global input should exist in interactive run")
            assert(type(input.prompt) == "function", "input.prompt")
            assert(type(input.confirm) == "function", "input.confirm")
            assert(type(input.select) == "function", "input.select")
        "#;
        let res = execute(cmd, &[], "", None, None, true);
        assert!(res.is_ok(), "interactive modules test failed: {res:?}");

        // ExecSilent & ExecPaged executions (allow_input = false) omit input module
        let non_interactive_cmd = r#"
            assert(input == nil, "input module must NOT be injected into non-interactive actions")
            assert(proc.editor == nil, "proc.editor must NOT be injected into non-interactive actions")
            assert(type(util) == "table", "util is still available")
            assert(type(lunacolors) == "table", "lunacolors is still available")
        "#;
        let non_interactive_res = execute(non_interactive_cmd, &[], "", None, None, false);
        assert!(
            non_interactive_res.is_ok(),
            "non-interactive module test failed: {non_interactive_res:?}"
        );
    }
}
