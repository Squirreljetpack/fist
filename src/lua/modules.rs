//! Lua built-in modules: the portable matchmaker set plus fist's own functions.
//!
//! [`matchmaker_mlua_bindings::register_modules`] installs `path`, `util`,
//! `proc`, `fs`, `lunacolors`, and (interactive only) `input` as globals and in
//! `package.preload`. The preload loaders return those exact tables, so the
//! fist functions appended below are reachable through `require(...)` as well.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

use mlua::{ExternalResult, Lua, Table, Value};

use crate::{
    lessfilter::{editor_cmd, mime_helpers::Myme},
    run::state::lessfilter_cfg,
};

/// Register the shared modules, then append fist's host functions to them.
pub fn register_modules(
    lua: &Lua,
    is_interactive: bool,
) -> mlua::Result<()> {
    matchmaker_mlua_bindings::register_modules(lua, is_interactive)?;

    let globals = lua.globals();
    append_path(&globals.get::<Table>("path")?, lua)?;
    append_util(&globals.get::<Table>("util")?, lua)?;
    append_fs(&globals.get::<Table>("fs")?, lua)?;
    append_proc(&globals.get::<Table>("proc")?, lua, is_interactive)?;
    Ok(())
}

/// `path.mime(path)` — the MIME type `mime:` conditions see, so Lua scripts
/// and condition filters agree.
fn append_path(
    t: &Table,
    lua: &Lua,
) -> mlua::Result<()> {
    let mime_fn = lua.create_function(|_, p: String| {
        Ok(
            Myme::from_path(Path::new(&p), lessfilter_cfg().settings.infer)
                .mime
                .map(|mime| mime.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string()),
        )
    })?;
    t.set("mime", mime_fn)
}

/// `util.clipboard_get()` and `util.clipboard_set(text, [opts])`.
fn append_util(
    t: &Table,
    lua: &Lua,
) -> mlua::Result<()> {
    let get_fn = lua.create_function(|_, ()| Ok(crate::clipboard::get_text()))?;
    t.set("clipboard_get", get_fn)?;

    let set_fn = lua.create_function(|_, (text, opts): (String, Option<Table>)| {
        let toast = match opts {
            Some(t) => t.get::<Option<bool>>("toast")?.unwrap_or(true),
            None => true,
        };
        crate::clipboard::copy_text(text, toast);
        Ok(())
    })?;
    t.set("clipboard_set", set_fn)
}

/// `fs.trash(path_or_paths) -> bool`.
fn append_fs(
    t: &Table,
    lua: &Lua,
) -> mlua::Result<()> {
    let trash_fn = lua.create_function(|_, val: Value| {
        let paths: Vec<PathBuf> = match val {
            Value::String(s) => vec![PathBuf::from(s.to_str()?.to_string())],
            Value::Table(t) => t
                .sequence_values::<String>()
                .map(|entry| entry.map(PathBuf::from))
                .collect::<mlua::Result<_>>()?,
            _ => {
                return Err(mlua::Error::RuntimeError(
                    "expected path string or table of paths".into(),
                ));
            }
        };
        for p in &paths {
            crate::utils::trash::trash(p).into_lua_err()?;
        }
        Ok(true)
    })?;
    t.set("trash", trash_fn)
}

/// `proc.editor([path]) -> { ok, code, editor }`, interactive executions only.
///
/// Runs the configured editor in the foreground with inherited stdio, so
/// full-screen editors and terminal input reach it directly. Background and
/// queued runs must not take over the terminal, hence the gate.
fn append_proc(
    t: &Table,
    lua: &Lua,
    is_interactive: bool,
) -> mlua::Result<()> {
    if !is_interactive {
        return Ok(());
    }
    let editor_fn = lua.create_function(|lua, path: Option<String>| {
        let mut words = editor_cmd();
        let prog = words.remove(0);
        let mut cmd = Command::new(&prog);
        cmd.args(&words);
        if let Some(path) = &path {
            cmd.arg(path);
        }
        let status = cmd
            .status()
            .map_err(|e| mlua::Error::RuntimeError(format!("failed to spawn '{prog}': {e}")))?;

        let res = lua.create_table()?;
        res.set("ok", status.success())?;
        res.set("code", status.code().unwrap_or(-1))?;
        res.set("editor", prog)?;
        Ok(res)
    })?;
    t.set("editor", editor_fn)
}
