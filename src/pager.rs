//! Lean pager: uncolored minus for command output and plain-text fallback.
//!
//! [`page_child`] pages a spawned command's stdout, [`page_reader`] any
//! stream, and [`page_file`] a file. `force_tty` (execute paths) draws on
//! `/dev/tty`; otherwise stdout is probed — a terminal runs interactive minus
//! on it, a pipe or file is streamed straight through without paging. The
//! pager never starts on empty output.

use std::{
    fs::File,
    io::{self, BufRead, BufReader, Cursor, Read, Write},
    path::Path,
    process::Child,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use cba::broc::TTY_HANDLE;
use log::error;
use minus::{LineNumbers, Pager, hooks::Hook, input::InputEvent};

/// Footer prompt shown by the pager.
const PROMPT: &str = "alt-h for help, q to quit";

/// Apply the pager defaults: no line numbers, smart-case search, the footer
/// prompt, and `y` to copy the selection to the clipboard.
fn configure_pager(pager: &Pager) {
    let _ = pager.set_line_numbers(LineNumbers::Disabled);
    let _ = pager.set_smart_case(true);
    let _ = pager.set_prompt(PROMPT);

    // Default bindings plus Alt-h help, Alt-w wrap toggle, horizontal mouse scroll, and `y` to copy.
    let mut input_register = minus::input::HashedEventRegister::default();
    input_register.add_help_key(&[]);
    input_register.add_described_key_events(&["m-w"], "toggle line wrap", |_, ps| {
        InputEvent::HorizontalScroll(!ps.screen.line_wrapping())
    });
    input_register.add_horizontal_mouse_scroll();
    let toast_pager = pager.clone();
    input_register.add_copy_key(&["y"], move |text| {
        crate::clipboard::copy_from_pager(text.to_string());
        let _ = toast_pager.send_toast("Copied!");
    });
    let _ = pager.set_selection_hint("y to copy, Esc to clear");
    let _ = pager.set_input_classifier(Box::new(input_register));
}

/// Neutralize minus's pre-populated `PostPagerExit` id-1 hook (`process::exit`)
/// and install id-2 to kill the paged child when the pager is quit early. The
/// hook only kills; [`page_child`] reaps the child afterwards, which a
/// take-and-reap in the hook would prevent.
fn configure_hooks(
    pager: &Pager,
    child: &Arc<Mutex<Option<Child>>>,
) {
    let _ = pager.remove_hook(Hook::PostPagerExit, 1);
    let _ = pager.add_hook(Hook::PostPagerExit, 1, Box::new(|_| {}));
    let child_hook = Arc::clone(child);
    let _ = pager.add_hook(
        Hook::PostPagerExit,
        2,
        Box::new(move |_| kill_child(&child_hook)),
    );
}

/// Kill a still-running child without reaping it, leaving the [`Child`] in
/// place so the caller can wait on it and read its exit status.
fn kill_child(slot: &Mutex<Option<Child>>) {
    if let Some(child) = slot.lock().unwrap().as_mut() {
        let _ = child.kill();
    }
}

/// Grace period before killing a paged child that kept running after stdout closed.
const CHILD_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

/// Wait up to `timeout` for `child` to exit; if it outlives the grace period,
/// kill and reap it. Returns whether it exited successfully.
fn wait_with_timeout(
    mut child: Child,
    timeout: Duration,
) -> io::Result<bool> {
    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(status.success()),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                return Ok(child.wait()?.success());
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Page a spawned child's stdout, or wait for a child that has none. Returns
/// once the child has been reaped; quitting the pager early kills it through
/// the `PostPagerExit` hook, and a child that lingers past stdout EOF is
/// killed after [`CHILD_WAIT_TIMEOUT`].
pub fn page_child(mut child: Child) -> io::Result<bool> {
    let Some(stdout) = child.stdout.take() else {
        return Ok(child.wait()?.success());
    };
    let slot = Arc::new(Mutex::new(Some(child)));
    let paged = page_into(stdout, true, Some(Arc::clone(&slot)));
    // The hook killed the child when the pager exited early; otherwise wait
    // briefly for it to exit as its stdout pipe closed, then kill and reap it.
    let child = slot
        .lock()
        .unwrap()
        .take()
        .expect("child remains in slot until reaped");
    let status_ok = wait_with_timeout(child, CHILD_WAIT_TIMEOUT)?;
    paged.map(|shown| shown && status_ok)
}

/// Page any reader. `force_tty` draws on `/dev/tty` (the caller owns stdout);
/// with `force_tty = false` a non-terminal stdout is streamed through unpaged.
pub fn page_reader<R: Read>(
    r: R,
    force_tty: bool,
) -> io::Result<()> {
    page_into(r, force_tty, None).map(|_| ())
}

/// Page a file to the current process's stdout — the lessfilter fallback when
/// `mm` is not on `PATH`.
pub fn page_file(path: &Path) -> io::Result<()> {
    let file =
        File::open(path).inspect_err(|e| error!("pager: cannot open {}: {e}", path.display()))?;
    page_reader(file, false)
}

/// Shared implementation of [`page_child`] and [`page_reader`]. Returns `false`
/// when `force_tty` could not attach to `/dev/tty`.
fn page_into<R: Read>(
    source: R,
    force_tty: bool,
    child: Option<Arc<Mutex<Option<Child>>>>,
) -> io::Result<bool> {
    // Never start a pager on empty output: probe the first line and hand it
    // back to the feed.
    let mut feed = BufReader::new(source);
    let mut first_line = Vec::new();
    if feed.read_until(b'\n', &mut first_line)? == 0 {
        return Ok(true);
    }
    let mut feed = Cursor::new(first_line).chain(feed);

    if force_tty || atty::is(atty::Stream::Stdout) {
        page_tty(feed, force_tty, child)
    } else {
        // stdout is not a terminal: no paging — stream straight through.
        let mut stdout = io::stdout().lock();
        io::copy(&mut feed, &mut stdout)?;
        stdout.flush()?;
        Ok(true)
    }
}

/// Interactive minus. With `force_tty` the output sink is `/dev/tty`;
/// otherwise minus keeps its default stdout sink, which the caller verified is
/// a terminal. Streams feed incrementally. Returns `false` when `force_tty`
/// could not attach to `/dev/tty`.
fn page_tty(
    mut source: impl Read,
    force_tty: bool,
    child: Option<Arc<Mutex<Option<Child>>>>,
) -> io::Result<bool> {
    let pager = Pager::new();
    configure_pager(&pager);
    if force_tty {
        let tty = TTY_HANDLE.as_ref().and_then(|tty| tty.try_clone().ok());
        let Some(tty) = tty else {
            // No /dev/tty: don't run minus; drop the output.
            error!("pager: /dev/tty is unavailable; dropping output");
            io::copy(&mut source, &mut io::sink())?;
            return Ok(false);
        };
        if let Err(e) = pager.set_output_sink(tty) {
            error!("pager: failed setting minus output sink: {e}");
            io::copy(&mut source, &mut io::sink())?;
            return Ok(false);
        }
    }
    if let Some(child) = child {
        configure_hooks(&pager, &child);
    }

    let pager_for_thread = pager.clone();
    let pager_thread = thread::spawn(move || {
        let _ = minus::dynamic_paging(pager_for_thread);
    });

    let mut feed = BufReader::new(source);
    let mut line = Vec::new();
    loop {
        line.clear();
        match feed.read_until(b'\n', &mut line) {
            Ok(0) => break, // EOF: the command finished (or was killed)
            Ok(_) => {
                let text = String::from_utf8_lossy(&line).into_owned();
                if pager.push_str(text).is_err() {
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
    let _ = pager.end_data();
    let _ = pager_thread.join();
    Ok(true)
}
