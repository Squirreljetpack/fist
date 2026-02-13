//! random utilities

use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::{cell::OnceCell, fs::File};

use cli_boilerplate_automation::bait::ResultExt;
use cli_boilerplate_automation::bo::map_reader_lines;
use cli_boilerplate_automation::broc::CommandExt;
use cli_boilerplate_automation::text::TableBuilder;
use cli_boilerplate_automation::{StringError, wbog};
use cli_boilerplate_automation::{bo::MapReaderError, bog::BogOkExt, broc::has, prints, vec_};
use crossterm::style::Stylize;

use crate::cli::paths::{current_exe, text_renderer_path};
use crate::lessfilter::mime_helpers::{detect_encoding, is_native};

#[allow(clippy::ptr_arg)]
pub fn is_header(cmd: &Vec<OsString>) -> bool {
    cmd.is_empty()
}

pub fn header_viewer(path: &Path) -> Vec<OsString> {
    vec_![]
}

#[allow(clippy::ptr_arg)]
pub fn is_metadata(cmd: &Vec<OsString>) -> bool {
    cmd.len() == 1 && cmd[0].is_empty()
}

pub fn metadata_viewer(path: &Path) -> Vec<OsString> {
    vec_![""]
}

// don't show header when printing to tty
pub fn show_header(path: &Path) {
    if !atty::is(atty::Stream::Stdout) {
        println!("{}", path.to_string_lossy().italic().dim());
        println!();
    }
}

// todo: in-house file -bL
pub fn show_metadata(
    path: &Path,
    first: bool,
) -> bool {
    if path.is_file() {
        if has("file") {
            let mut cmd = Command::new("file");

            // custom extra info
            let _path = path.to_path_buf();

            let join = detect_encoding(path)
                .as_deref()
                .is_some_and(is_native)
                .then_some(std::thread::spawn(move || count_file(_path)));

            let ret = cmd
                .arg("-bL")
                .arg(path)
                // .stderr(Stdio::null())
                .output()
                ._ebog();

            if let Some(s) = &ret {
                let s = String::from_utf8_lossy(&s.stdout);
                if !atty::is(atty::Stream::Stdout) && !first {
                    println!("\n");
                }
                print!("{}", s.dim().italic())
            };

            if let Some(join) = join {
                match join.join() {
                    Ok(Ok(counts)) => {
                        let mut table = TableBuilder::new([11, 10, 9])
                            .header(["chars", "words", "lines"])
                            .separator("—".repeat(30).dim().italic())
                            .row(counts)
                            .header_formatter(|s, _| s.dim().italic().to_string())
                            .cell_formatter(|s, _| s.dim().italic().to_string());

                        if ret.is_some() || !first {
                            println!("\n");
                        }
                        table.print();
                    }
                    _ => {
                        // error
                    }
                }
            }
            ret.is_some_and(|o| o.status.success())
        } else {
            eprintln!("Error: The built-in metadata viewer requires the 'file' program.",);
            false
        }
    } else {
        let mut cmd = Command::new(current_exe());
        cmd.args([
            ":tool", "liza", ":l", //"--no-header",
            "--",
        ])
        .arg(path)
        .status()
        ._ebog()
        .is_some_and(|s| s.success())
    }
}

fn count_file<P: AsRef<Path>>(path: P) -> Result<[usize; 3], MapReaderError<StringError>> {
    let path = path.as_ref();
    let file = File::open(path).prefix(format!("Failed to open {}", path.to_string_lossy()))?;

    let mut chars = 0usize;
    let mut words = 0usize;
    let mut lines = 0usize;
    let mut in_word = false;

    map_reader_lines::<true, _>(file, |line| {
        lines += 1;

        for c in line.chars() {
            chars += 1;
            if c.is_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                words += 1;
            }
        }

        in_word = false;
        Ok(())
    })?;

    Ok([chars, words, lines])
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

// ---------------- EXTRACT -----------------

// todo: maybe we inhouse this with some kind of extractor library
// although kreuzberg seems a bit overweight if using as a lib
pub fn extract(path: &Path) -> bool {
    if !has("kreuzberg") {
        wbog!("Action::Extract requires the 'kreuzberg' command.");
        return false;
    }

    let Some(kreuzberg) = Command::new("kreuzberg")
        .args(["extract", "--output-format=plain"])
        .arg(path)
        .stdout(Stdio::piped())
        .spawn_piped()
        ._elog()
    else {
        return false;
    };

    let mut pager = Command::new(text_renderer_path());

    pager.stdin(kreuzberg);

    pager.status()._ebog().is_some_and(|s| s.success())
}
