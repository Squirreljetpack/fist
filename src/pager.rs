//! Rust pager: optional bat passthrough into minus.
//!
//! `page_child` pages a spawned child's stdout, drawing on `/dev/tty`
//! (execute paths). `page_reader` and `render_text` render a stream or file
//! with `force_tty=false`: stdout is probed — a terminal runs interactive
//! minus on it, a pipe or file is passed straight through without paging.
//! `render_text` opens a file and pages it to the subtool's own stdout.
//!
//! When `bat` is `Some(opts)` and the `bat` binary exists, the stream is piped
//! through it first. The pager never starts on empty output (first-line gate).

use std::{
    fs::File,
    io::{self, BufRead, BufReader, Cursor, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::config::pager_cfg;
use cba::broc::{has, TTY_HANDLE};
use log::error;
use minus::{hooks::Hook, LineNumbers, Pager};

/// Poll a child's exit status for up to `timeout`, then kill it; returns whether
/// it exited successfully.
fn wait_with_timeout(
    mut child: Child,
    timeout: Duration,
) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                error!("pager: failed polling child status: {e}");
                return false;
            }
        }
    }
}

/// Reap the still-present children and report whether the paged output
/// completed: the command child's exit status when one was spawned, else the
/// bat child's. `false` when that child was killed (early quit or timeout) or
/// exited non-zero; `true` when there is no child to wait on.
fn child_status(
    cmd_child: &Mutex<Option<Child>>,
    bat_child: &Mutex<Option<Child>>,
) -> bool {
    let cmd = cmd_child.lock().unwrap().take();
    if let Some(child) = cmd {
        return wait_with_timeout(child, Duration::from_secs(5));
    }
    // include bat's exit status when no command child exists so that missing files propogate errors
    let bat = bat_child.lock().unwrap().take();
    if let Some(child) = bat {
        return wait_with_timeout(child, Duration::from_secs(5));
    }
    true
}

/// Kill a still-running child in `slot` without reaping it, leaving the
/// [`Child`] in place so the caller can reap it and read its exit status.
fn kill_child(slot: &Mutex<Option<Child>>) {
    if let Some(child) = slot.lock().unwrap().as_mut() {
        let _ = child.kill();
    }
}

/// Apply the `pager.toml` config to a minus pager: line numbers, follow mode,
/// horizontal scroll, smart case search, and the footer prompt (default
/// `/ or ? to search`). Loaded through [`pager_cfg`], which works in subtool
/// processes too (no `GLOBAL::init` needed).
fn configure_pager(pager: &Pager) {
    let cfg = pager_cfg();
    let _ = pager.set_line_numbers(if cfg.line_numbers {
        LineNumbers::Enabled
    } else {
        LineNumbers::Disabled
    });
    if cfg.follow {
        let _ = pager.follow_output(true);
    }
    if cfg.horizontal_scroll {
        let _ = pager.horizontal_scroll(true);
    }
    let _ = pager.set_smart_case(cfg.smart_case);
    let prompt = cfg
        .prompt
        .clone()
        .unwrap_or_else(|| "alt-h for help, q to quit".to_string());
    let _ = pager.set_prompt(prompt);

    // Default bindings plus the Alt-h help binding.
    let mut input_register = minus::input::HashedEventRegister::default();
    input_register.add_help_key(&[]);
    let _ = pager.set_input_classifier(Box::new(input_register));

    // Route pager selection copies through fist's existing clipboard handle
    // instead of a fresh arboard connection.
    let _ = pager.set_clipboard_handler(Box::new(|text| {
        crate::clipboard::copy_from_pager(text.to_string());
    }));
}

/// Neutralize minus's pre-populated `PostPagerExit` id-1 hook (`process::exit`)
/// and install id-2 to kill the command/bat children on early quit. The hook
/// only kills — [`page_tty`] reaps the children afterwards and reads the
/// command's exit status, which a take-and-reap here would erase.
fn configure_hooks(
    pager: &Pager,
    cmd_child: &Arc<Mutex<Option<Child>>>,
    bat_child: &Arc<Mutex<Option<Child>>>,
) {
    let _ = pager.remove_hook(Hook::PostPagerExit, 1);
    let _ = pager.add_hook(Hook::PostPagerExit, 1, Box::new(|_| {}));
    let cmd_hook = cmd_child.clone();
    let bat_hook = bat_child.clone();
    let _ = pager.add_hook(
        Hook::PostPagerExit,
        2,
        Box::new(move |_| {
            kill_child(&cmd_hook);
            kill_child(&bat_hook);
        }),
    );
}

/// Spawn `bat <opts> [path]` with piped stdin/stdout. With a path, bat opens the
/// file itself (language detection works) and stdin stays unused; without one,
/// the caller feeds the content on a writer thread so a failed spawn can fall
/// back to the raw stream.
fn spawn_bat(
    opts: Vec<String>,
    path: Option<&Path>,
) -> io::Result<(Child, ChildStdin, ChildStdout)> {
    let mut cmd = Command::new("bat");
    cmd.args(&opts).stdin(Stdio::piped()).stdout(Stdio::piped());
    if let Some(path) = path {
        cmd.arg("--").arg(path);
    }
    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take().expect("bat stdin is piped");
    let stdout = child.stdout.take().expect("bat stdout is piped");
    Ok((child, stdin, stdout))
}

/// Open `path` as the pager's read source.
fn open_source(path: &Path) -> io::Result<Box<dyn Read + Send>> {
    File::open(path)
        .inspect_err(|e| error!("pager: cannot open {}: {e}", path.display()))
        .map(|f| Box::new(f) as Box<dyn Read + Send>)
}

/// Page a spawned child's stdout. Minus's output sink is `/dev/tty` (from
/// `cba::broc::TTY_HANDLE`); when the handle is absent or not cloneable, minus
/// is not run — the stream is drained and `Ok(false)` returned. `bat`:
/// Some(opts) → pipe through `bat` first if the binary exists.
///
/// Returns whether the child exited successfully before the pipe closed
/// (`false` on empty output, early quit, or a killed child); drives the DB bump.
pub fn page_child(
    mut child: Child,
    bat: Option<Vec<String>>,
) -> io::Result<bool> {
    let stdout = child
        .stdout
        .take()
        .expect("paged child stdout must be piped");
    page_inner(Ok(Box::new(stdout)), Some(child), true, bat)
}

/// Render any reader (no child). `force_tty=false` for the subtool paths:
/// stdout is probed — a terminal gets interactive minus, a pipe or file is
/// passed straight through without paging. Returns whether the stream was
/// non-empty.
pub fn page_reader<R: Read + Send + 'static>(
    r: R,
    force_tty: bool,
    bat: Option<Vec<String>>,
) -> io::Result<bool> {
    page_inner(Ok(Box::new(r)), None, force_tty, bat)
}

/// Render a file to the current process's stdout (the subtool's stdout when run
/// from the lessfilter executor). Bat args come from the subtool env logic. The
/// path goes to bat directly (no first-line gate — the file is known to exist).
/// Returns whether the file rendered; `Err` when it cannot be opened and
/// `Ok(false)` when bat fails to render it.
pub fn render_text(
    path: &Path,
    bat: Option<Vec<String>>,
) -> io::Result<bool> {
    page_inner(Err(path.to_path_buf()), None, false, bat)
}

/// The pager input is either a stream (`Ok`) or a file path (`Err`).
///
/// Stream input: the first-line gate runs here — never start a pager on empty
/// output. Bat receives the stream on stdin.
///
/// Path input: the gate is skipped — the path is passed to bat directly (bat
/// opens the file itself, so language detection works); without bat (or when it
/// fails to spawn) the file is opened in-process. A path that cannot be opened
/// is `Err`.
fn page_inner(
    input: Result<Box<dyn Read + Send>, PathBuf>,
    child: Option<Child>,
    force_tty: bool,
    bat: Option<Vec<String>>,
) -> io::Result<bool> {
    let mut bat_child: Option<Child> = None;
    let mut feed: Box<dyn Read + Send> = match input {
        Ok(reader) => {
            let mut buf = BufReader::new(reader);
            let mut first_line = String::new();
            if buf.read_line(&mut first_line)? == 0 {
                return Ok(false); // empty: no pager, no reaping, no bump
            }
            let raw: Box<dyn Read + Send> =
                Box::new(Cursor::new(first_line.into_bytes()).chain(buf));
            if let Some(opts) = bat.filter(|_| has("bat")) {
                match spawn_bat(opts, None) {
                    Ok((child, mut stdin, stdout)) => {
                        let mut source = raw;
                        thread::spawn(move || {
                            let _ = io::copy(&mut source, &mut stdin);
                        });
                        bat_child = Some(child);
                        Box::new(stdout)
                    }
                    Err(e) => {
                        error!("pager: failed spawning bat: {e}");
                        raw
                    }
                }
            } else {
                raw
            }
        }
        Err(path) => match bat.filter(|_| has("bat")) {
            Some(opts) => match spawn_bat(opts, Some(&path)) {
                Ok((child, _stdin, stdout)) => {
                    bat_child = Some(child);
                    Box::new(stdout)
                }
                Err(e) => {
                    error!("pager: failed spawning bat: {e}");
                    // bat failed to spawn: open the file in-process instead
                    open_source(&path)?
                }
            },
            None => open_source(&path)?,
        },
    };

    let cmd_child = Arc::new(Mutex::new(child));
    let bat_child = Arc::new(Mutex::new(bat_child));

    if force_tty || atty::is(atty::Stream::Stdout) {
        // Interactive minus: on `/dev/tty` when forced (execute paths), else on
        // the default stdout sink (the caller's stdout is a terminal, e.g.
        // `:tool pager` run from a shell).
        page_tty(feed, &cmd_child, &bat_child, force_tty)
    } else {
        // stdout is not a terminal: no paging — stream straight through
        // (bat-colored when bat ran).
        let mut stdout = io::stdout().lock();
        io::copy(&mut feed, &mut stdout)?;
        stdout.flush()?;
        // Reap children still around; they exit as the pipe closed.
        Ok(child_status(&cmd_child, &bat_child))
    }
}

/// Interactive minus. With `force_tty` the output sink is `/dev/tty` (from
/// `cba::broc::TTY_HANDLE`); otherwise minus keeps its default stdout sink,
/// which the caller verified is a terminal. Streams feed incrementally.
fn page_tty(
    mut source: Box<dyn Read + Send>,
    cmd_child: &Arc<Mutex<Option<Child>>>,
    bat_child: &Arc<Mutex<Option<Child>>>,
    force_tty: bool,
) -> io::Result<bool> {
    let pager = Pager::new();
    configure_pager(&pager);
    if force_tty {
        let tty = TTY_HANDLE.as_ref().and_then(|f| f.try_clone().ok());
        let Some(tty) = tty else {
            // No /dev/tty: don't run minus; drain and report failure.
            error!("pager: tty requested but /dev/tty is unavailable; dropping output");
            let mut sink = io::sink();
            io::copy(&mut source, &mut sink)?;
            return Ok(false);
        };
        if let Err(e) = pager.set_output_sink(tty) {
            error!("pager: failed setting minus output sink: {e}");
            let mut sink = io::sink();
            io::copy(&mut source, &mut sink)?;
            return Ok(false);
        }
    }
    configure_hooks(&pager, cmd_child, bat_child);

    let pager_for_thread = pager.clone();
    let pager_thread = thread::spawn(move || {
        let _ = minus::dynamic_paging(pager_for_thread);
    });

    let mut feed = BufReader::new(source);
    let mut line = String::new();
    loop {
        line.clear();
        match feed.read_line(&mut line) {
            Ok(0) => break, // EOF: the command finished (or was killed)
            Ok(_) => {
                if pager.push_str(line.clone()).is_err() {
                    break; // pager quit; stop feeding
                }
            }
            Err(e) => {
                error!("pager: failed reading output: {e}");
                break;
            }
        }
    }
    drop(feed);
    let _ = pager_thread.join();

    // The id-2 hook killed the children when the pager exited early; otherwise
    // they exited as the pipe closed. Reap them and report the command's exit
    // status (bat's when no command ran) — Ok(false) when killed, non-zero, or
    // outlived the wait.
    Ok(child_status(cmd_child, bat_child))
}
