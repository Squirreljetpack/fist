//! Shipped Lua modules for the action execution environment.

pub mod fs;
pub mod input;
pub mod lunacolors;
pub mod path;
pub mod proc;
pub mod util;

use mlua::{Lua, Table};

/// Register a module both as a global and in `package.preload` for
/// `require(...)`.
fn install_module(
    lua: &Lua,
    name: &str,
    module: Table,
) -> mlua::Result<()> {
    lua.globals().set(name, module.clone())?;
    let package: Table = lua.globals().get("package")?;
    let preload: Table = package.get("preload")?;
    preload.set(name, lua.create_function(move |_, ()| Ok(module.clone()))?)?;
    Ok(())
}

/// Register the bundled `lunacolors` library as global `lunacolors` (and
/// `Lunacolors`) and in `package.preload`.
pub fn register_lunacolors(lua: &Lua) -> mlua::Result<()> {
    let module = lunacolors::create_module(lua)?;
    install_module(lua, "lunacolors", module.clone())?;
    install_module(lua, "Lunacolors", module)
}

/// Register the bundled `path`, `util`, `proc`, `fs`, and (if interactive)
/// `input` modules both as globals and in `package.preload` for `require(...)`.
pub fn register_modules(
    lua: &Lua,
    is_interactive: bool,
) -> mlua::Result<()> {
    install_module(lua, "path", path::create_module(lua)?)?;
    install_module(lua, "util", util::create_module(lua)?)?;
    install_module(lua, "proc", proc::create_module(lua, is_interactive)?)?;
    install_module(lua, "fs", fs::create_module(lua)?)?;
    register_lunacolors(lua)?;

    // input and proc.editor only in interactive blocking environments
    if is_interactive {
        install_module(lua, "input", input::create_module(lua)?)?;
    }

    Ok(())
}
