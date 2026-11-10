//! Native filesystem and metadata operations exposed to the Lua VM (`fs.*`).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use mlua::{ExternalResult, Lua, Table, Value};

pub fn create_module(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;

    // fs.stat(path) -> table | nil
    let stat_fn = lua.create_function(|lua, p: String| {
        let path = Path::new(&p);
        let md = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(None),
        };
        let sym_md = fs::symlink_metadata(path).ok();
        let is_symlink = sym_md.as_ref().map(|s| s.is_symlink()).unwrap_or(false);

        let res = lua.create_table()?;
        res.set("size", md.len())?;
        res.set("is_file", md.is_file())?;
        res.set("is_dir", md.is_dir())?;
        res.set("is_symlink", is_symlink)?;
        res.set("readonly", md.permissions().readonly())?;

        let mtime = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        res.set("mtime", mtime)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = md.permissions().mode();
            res.set("mode", mode)?;
            res.set("executable", mode & 0o111 != 0)?;
        }
        #[cfg(not(unix))]
        {
            res.set("mode", 0u32)?;
            res.set("executable", false)?;
        }

        Ok(Some(res))
    })?;
    t.set("stat", stat_fn)?;

    // fs.chmod(path, mode) -> bool
    let chmod_fn = lua.create_function(|_, (p, mode_val): (String, Value)| {
        let path = Path::new(&p);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let new_mode = match mode_val {
                Value::Integer(n) => n as u32,
                Value::Number(n) => n as u32,
                Value::String(s) => {
                    let str_val = s.to_str()?;
                    if str_val == "+x" || str_val == "-x" {
                        let curr = fs::metadata(path).into_lua_err()?.permissions().mode();
                        if str_val == "+x" {
                            curr | 0o111
                        } else {
                            curr & !0o111
                        }
                    } else {
                        u32::from_str_radix(str_val.trim_start_matches("0o"), 8).map_err(|_| {
                            mlua::Error::RuntimeError(format!("invalid chmod mode: {str_val}"))
                        })?
                    }
                }
                _ => {
                    return Err(mlua::Error::RuntimeError(
                        "mode must be an integer, octal string, '+x', or '-x'".into(),
                    ));
                }
            };
            fs::set_permissions(path, fs::Permissions::from_mode(new_mode)).into_lua_err()?;
            Ok(true)
        }
        #[cfg(not(unix))]
        {
            let _ = (path, mode_val);
            Ok(false)
        }
    })?;
    t.set("chmod", chmod_fn)?;

    // fs.mkdir(path, [recursive = true]) -> bool
    let mkdir_fn = lua.create_function(|_, (p, recursive): (String, Option<bool>)| {
        let path = Path::new(&p);
        if recursive.unwrap_or(true) {
            fs::create_dir_all(path).into_lua_err()?;
        } else {
            fs::create_dir(path).into_lua_err()?;
        }
        Ok(true)
    })?;
    t.set("mkdir", mkdir_fn)?;

    // fs.readdir(path) -> table of strings
    let readdir_fn = lua.create_function(|lua, p: String| {
        let path = Path::new(&p);
        let rd = fs::read_dir(path).into_lua_err()?;
        let res = lua.create_table()?;
        for (i, e) in rd.flatten().enumerate() {
            res.raw_seti(i + 1, e.file_name().to_string_lossy().into_owned())?;
        }
        Ok(res)
    })?;
    t.set("readdir", readdir_fn)?;

    // fs.read(path) -> string
    let read_fn =
        lua.create_function(|_, p: String| fs::read_to_string(Path::new(&p)).into_lua_err())?;
    t.set("read", read_fn)?;

    // fs.write(path, content) -> bool
    let write_fn = lua.create_function(|_, (p, content): (String, String)| {
        fs::write(Path::new(&p), content).into_lua_err()?;
        Ok(true)
    })?;
    t.set("write", write_fn)?;

    // fs.remove(path, [recursive = false]) -> bool
    let remove_fn = lua.create_function(|_, (p, recursive): (String, Option<bool>)| {
        let path = Path::new(&p);
        if recursive.unwrap_or(false) {
            if path.is_dir() {
                fs::remove_dir_all(path).into_lua_err()?;
            } else {
                fs::remove_file(path).into_lua_err()?;
            }
        } else if path.is_dir() {
            fs::remove_dir(path).into_lua_err()?;
        } else {
            fs::remove_file(path).into_lua_err()?;
        }
        Ok(true)
    })?;
    t.set("remove", remove_fn)?;

    // fs.copy(src, dst) -> bool
    let copy_fn = lua.create_function(|_, (src, dst): (String, String)| {
        fs::copy(Path::new(&src), Path::new(&dst)).into_lua_err()?;
        Ok(true)
    })?;
    t.set("copy", copy_fn)?;

    // fs.rename(src, dst) -> bool
    let rename_fn = lua.create_function(|_, (src, dst): (String, String)| {
        fs::rename(Path::new(&src), Path::new(&dst)).into_lua_err()?;
        Ok(true)
    })?;
    t.set("rename", rename_fn)?;

    // fs.symlink(src, dst) -> bool
    let symlink_fn = lua.create_function(|_, (src, dst): (String, String)| {
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(Path::new(&src), Path::new(&dst)).into_lua_err()?;
            Ok(true)
        }
        #[cfg(windows)]
        {
            let src_p = Path::new(&src);
            if src_p.is_dir() {
                std::os::windows::fs::symlink_dir(src_p, Path::new(&dst)).into_lua_err()?;
            } else {
                std::os::windows::fs::symlink_file(src_p, Path::new(&dst)).into_lua_err()?;
            }
            Ok(true)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(mlua::Error::RuntimeError(
                "symlink unsupported on platform".into(),
            ))
        }
    })?;
    t.set("symlink", symlink_fn)?;

    // fs.readlink(path) -> string
    let readlink_fn = lua.create_function(|_, p: String| {
        fs::read_link(Path::new(&p))
            .map(|pb| pb.to_string_lossy().into_owned())
            .into_lua_err()
    })?;
    t.set("readlink", readlink_fn)?;

    // fs.canonicalize(path) -> string
    let canonicalize_fn = lua.create_function(|_, p: String| {
        fs::canonicalize(Path::new(&p))
            .map(|pb| pb.to_string_lossy().into_owned())
            .into_lua_err()
    })?;
    t.set("canonicalize", canonicalize_fn)?;

    // fs.trash(path_or_paths) -> bool
    let trash_fn = lua.create_function(|_, val: Value| {
        let paths: Vec<PathBuf> = match val {
            Value::String(s) => vec![PathBuf::from(s.to_str()?.to_string())],
            Value::Table(tbl) => {
                let mut list = Vec::new();
                for entry in tbl.sequence_values::<String>() {
                    list.push(PathBuf::from(entry?));
                }
                list
            }
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
    t.set("trash", trash_fn)?;

    // fs.tempdir([prefix]) -> string
    let tempdir_fn = lua.create_function(|_, prefix: Option<String>| {
        fresh_temp_path(&prefix, |p| fs::create_dir(p)).into_lua_err()
    })?;
    t.set("tempdir", tempdir_fn)?;

    // fs.tempfile([prefix]) -> string
    let tempfile_fn = lua.create_function(|_, prefix: Option<String>| {
        fresh_temp_path(&prefix, |p| {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(p)
                .map(drop)
        })
        .into_lua_err()
    })?;
    t.set("tempfile", tempfile_fn)?;

    Ok(t)
}

/// Create an unused path under the temp dir by attempting `create` until it
/// does not collide with an existing entry.
fn fresh_temp_path(
    prefix: &Option<String>,
    create: impl Fn(&Path) -> std::io::Result<()>,
) -> std::io::Result<String> {
    let prefix = prefix.as_deref().unwrap_or("fist_tmp_");
    for _ in 0..64 {
        let p = std::env::temp_dir().join(format!("{prefix}{:x}", fastrand::u64(..)));
        match create(&p) {
            Ok(()) => return Ok(p.to_string_lossy().into_owned()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::other("could not create a unique temp path"))
}
