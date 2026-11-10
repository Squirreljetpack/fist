//! Bundled `lunacolors` styling library.

use mlua::{Lua, Table};

pub fn create_module(lua: &Lua) -> mlua::Result<Table> {
    lua.load(include_str!("lunacolors.lua")).eval()
}
