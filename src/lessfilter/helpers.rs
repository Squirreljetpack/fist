//! random utilities

use std::cell::OnceCell;
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use cli_boilerplate_automation::vec_;

#[allow(clippy::ptr_arg)]
pub fn is_header(cmd: &Vec<OsString>) -> bool {
    cmd.is_empty()
}

pub fn header_viewer(path: &Path) -> Vec<OsString> {
    vec_![]
}

pub fn show_header(path: &Path) {
    println!("\x1b[3;2m{}\x1b[0m\n", path.display());
}

// ----------------- IMAGE ---------------------------------

thread_local! {
    // vs lazycell?
    static CHAFA_FORMAT: OnceCell<&'static str> = const { OnceCell::new() };
    static VISUAL: OnceCell<Vec<String>> = const { OnceCell::new() };
}

fn infer_chafa_format() -> &'static str {
    CHAFA_FORMAT.with(|cell| {
        *cell.get_or_init(|| {
            if let Ok(v) = env::var("CHAFA_FORMAT") {
                if !v.is_empty() {
                    return Box::leak(v.into_boxed_str());
                }
            }

            if env::var_os("KITTY_WINDOW_ID").is_some() {
                return "kitty";
            }

            if env::var_os("ITERM_SESSION_ID").is_some() {
                return "iterm";
            }

            let sixel = Command::new("infocmp")
                .output()
                .ok()
                .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("sixel"))
                .unwrap_or(false);

            if sixel { "sixels" } else { "symbols" }
        })
    })
}

pub fn image_viewer(path: &Path) -> Vec<OsString> {
    vec_!["chafa", "-f", infer_chafa_format(), path]
}

pub fn infer_visual(path: &Path) -> Vec<OsString> {
    VISUAL.with(|cell| {
        cell.get_or_init(|| {
            if let Ok(v) = env::var("FS_VISUAL") {
                if !v.is_empty()
                    && let Ok(words) = shell_words::split(&v)
                {
                    return words;
                }
            }

            if let Ok(v) = env::var("VISUAL") {
                if !v.is_empty()
                    && let Ok(words) = shell_words::split(&v)
                {
                    return words;
                }
            }

            // use our default opener
            vec_!["fs", ":open"]
        })
        .iter()
        .map(OsString::from)
        .chain(std::iter::once(path.into()))
        .collect()
    })
}

// ---------------------- EDITOR -----------------------------

pub static LINE_COLUMN: Mutex<Option<(usize, usize)>> = const { Mutex::new(None) };
pub fn infer_editor(path: &Path) -> Vec<OsString> {
    // get base_cmd
    let base_cmd: Vec<String> = if let Ok(v) = env::var("FS_EDITOR") {
        if !v.is_empty() {
            shell_words::split(&v).unwrap_or_default()
        } else {
            Vec::new()
        }
    } else if let Ok(v) = env::var("EDITOR") {
        if !v.is_empty() {
            shell_words::split(&v).unwrap_or_default()
        } else {
            Vec::new()
        }
    } else {
        vec!["fs".into(), ":open".into()]
    };

    let mut cmd: Vec<OsString> = base_cmd.iter().map(OsString::from).collect();

    // Try to apply line/column if supported
    let editor_name = cmd.first().and_then(|s| s.to_str()).unwrap_or_default();

    let line_col: Option<(usize, usize)> = {
        let guard = LINE_COLUMN.lock().unwrap();
        if guard.is_some() {
            *guard
        } else if let Ok(v) = env::var("FS_LINE_COLUMN") {
            parse_line_column(&v)
        } else {
            None
        }
    };

    if let Some((line, col)) = line_col {
        match editor_name {
            "micro" => {
                let s = format!("{}:{}", path.display(), line);
                cmd.push(OsString::from(s));
            }
            "vim" | "nvim" => {
                cmd.push(OsString::from(format!("+{}", line)));
                cmd.push(path.into());
            }
            "nano" => {
                cmd.push(OsString::from(format!("+{},{}", line, col)));
                cmd.push(path.into());
            }
            _ => {
                cmd.push(path.into());
            }
        }
    } else {
        cmd.push(path.into());
    }

    cmd
}

/// Parse line/column string like "10:3" or "10,3"
fn parse_line_column(s: &str) -> Option<(usize, usize)> {
    if let Some((l, c)) = s.split_once(':') {
        Some((l.parse().ok()?, c.parse().ok()?))
    } else if let Some((l, c)) = s.split_once(',') {
        Some((l.parse().ok()?, c.parse().ok()?))
    } else {
        None
    }
}
