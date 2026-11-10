//! Terminal input functions exposed to the Lua VM (`input.*`).
//!
//! Only active in interactive blocking execution environments.

use std::io::{BufRead, BufReader, Write};

use matchmaker::nucleo::Worker;
use matchmaker::{MatchError, Matchmaker, PickOptions};
use mlua::{Lua, LuaSerdeExt, Table, Value};
use ratatui::text::Line;

pub fn create_module(lua: &Lua) -> mlua::Result<Table> {
    let t = lua.create_table()?;

    // input.prompt(prompt_text, [default_val]) -> string | nil
    let prompt_fn = lua.create_function(|_, (msg, default): (String, Option<String>)| {
        let prompt_display = match &default {
            Some(d) => format!("{msg} [{d}]: "),
            None => format!("{msg} "),
        };

        print_tty(&prompt_display);
        let input = read_tty_line();

        match input {
            Some(line) if line.trim().is_empty() => Ok(default),
            Some(line) => Ok(Some(line)),
            None => Ok(default),
        }
    })?;
    t.set("prompt", prompt_fn)?;

    // input.confirm(prompt_text, [default_bool]) -> bool
    let confirm_fn = lua.create_function(|_, (msg, default): (String, Option<bool>)| {
        let hint = match default {
            Some(true) => "[Y/n]",
            Some(false) => "[y/N]",
            None => "[y/n]",
        };
        print_tty(&format!("{msg} {hint} "));
        let input = read_tty_line();

        match input {
            Some(line) if line.trim().is_empty() => Ok(default.unwrap_or(false)),
            Some(line) => {
                let first = line.trim().chars().next().unwrap_or('n');
                Ok(first == 'y' || first == 'Y')
            }
            None => Ok(default.unwrap_or(false)),
        }
    })?;
    t.set("confirm", confirm_fn)?;

    // input.select(items_table, [prompt_text]) -> string | nil
    let select_fn = lua.create_function(|lua, (items_val, prompt): (Value, Option<String>)| {
        let items: Vec<String> = lua.from_value(items_val).map_err(|e| {
            mlua::Error::RuntimeError(format!("items must be a list of strings: {e}"))
        })?;

        if items.is_empty() {
            return Ok(None);
        }

        Ok(select_with_matchmaker(items, prompt.as_deref()))
    })?;
    t.set("select", select_fn)?;

    Ok(t)
}

fn select_with_matchmaker(
    items: Vec<String>,
    prompt: Option<&str>,
) -> Option<String> {
    let worker = Worker::new_single_column();
    worker.append(items);
    let mm = Matchmaker::new_on_cloneable(worker);

    let pick_fut = async move {
        let res = if let Some(p) = prompt {
            let prompt_line = format!("{p} ");
            let options = PickOptions::<String, ()>::new().initializer(move |state| {
                state
                    .picker_ui
                    .query
                    .set_prompt_line(Line::from(prompt_line));
            });
            mm.pick(options).await
        } else {
            mm.pick_default().await
        };

        match res {
            Ok(v) => v.into_iter().next(),
            Err(MatchError::Abort(_)) => None,
            Err(e) => {
                log::warn!("Matchmaker select error: {e}");
                None
            }
        }
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(pick_fut))
    } else if let Ok(rt) = tokio::runtime::Runtime::new() {
        rt.block_on(pick_fut)
    } else {
        None
    }
}

fn print_tty(msg: &str) {
    #[cfg(unix)]
    {
        if let Ok(mut tty) = std::fs::OpenOptions::new().write(true).open("/dev/tty") {
            let _ = tty.write_all(msg.as_bytes());
            let _ = tty.flush();
            return;
        }
    }
    eprint!("{msg}");
    let _ = std::io::stderr().flush();
}

fn read_tty_line() -> Option<String> {
    #[cfg(unix)]
    {
        if let Ok(tty) = std::fs::File::open("/dev/tty") {
            let mut reader = BufReader::new(tty);
            let mut line = String::new();
            if reader.read_line(&mut line).is_ok() {
                return Some(line.trim_end_matches(['\r', '\n']).to_string());
            }
        }
    }
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_ok() {
        Some(line.trim_end_matches(['\r', '\n']).to_string())
    } else {
        None
    }
}
