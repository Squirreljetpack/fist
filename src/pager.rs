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
    io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use notify::{Config as NotifyConfig, Event, RecursiveMode, Watcher as NotifyWatcher};

use crate::config::pager_cfg;
use cba::broc::{TTY_HANDLE, has};
use log::error;
use minus::{LineNumbers, Pager, hooks::Hook};

/// Options controlling scroll position, follow mode, and fallback windowing
/// for non-TTY paths when bat is absent.
#[derive(Default)]
pub struct PagerOpts {
    /// 0-indexed line to scroll to on open (TTY path only).
    pub scroll_to: Option<usize>,
    /// Auto-scroll to the bottom as content arrives (follow / live-tail mode).
    pub follow: bool,
    /// Cancellation flag shared with a [`FileTailer`]; the PostPagerExit hook
    /// sets it to signal the tailer to stop.
    pub stop: Option<Arc<AtomicBool>>,
    /// Line window for the no-bat non-TTY fallback: `(start, end)` where both
    /// are 1-based and `end = None` means read to EOF.
    pub fallback_range: Option<(usize, Option<usize>)>,
}

/// Live-tailing reader for a file. Seeks to the end of the file on
/// construction, then blocks on each `read` call until new bytes arrive or the
/// `stop` flag is set.
///
/// Uses a [`notify::RecommendedWatcher`] (NonRecursive, watching the parent
/// directory) to wake immediately when the file is modified, rather than
/// sleeping on a fixed interval. Falls back to a 10 ms sleep when the watcher
/// cannot be started. Truncation / rotation is detected by comparing the
/// current file size to the last-read offset.
pub struct FileTailer {
    path: PathBuf,
    file: File,
    offset: u64,
    stop: Arc<AtomicBool>,
    /// Receives a signal each time the watched file is modified.
    rx: mpsc::Receiver<()>,
    /// Kept alive for the duration of the tailer; dropped when the tailer is
    /// dropped, which unregisters the watch.
    _watcher: Option<Box<dyn notify::Watcher + Send>>,
}

impl FileTailer {
    /// Open `path`, seek to its current end, and return a tailer. Content
    /// appended after this call will be visible on reads.
    pub fn new(
        path: &Path,
        stop: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let mut file = File::open(path)?;
        let offset = file.seek(SeekFrom::End(0))?;

        let (tx, rx) = mpsc::channel::<()>();

        // Watch the parent directory NonRecursively; filter events to our
        // file in the callback.
        let watch_path = path.to_path_buf();
        let _watcher: Option<Box<dyn notify::Watcher + Send>> =
            match notify::RecommendedWatcher::new(
                move |res: notify::Result<Event>| {
                    if let Ok(ev) = res {
                        use notify::EventKind::*;
                        if matches!(ev.kind, Modify(_) | Create(_))
                            && ev.paths.iter().any(|p| p == &watch_path)
                        {
                            let _ = tx.send(());
                        }
                    }
                },
                NotifyConfig::default(),
            ) {
                Ok(mut w) => {
                    let dir = path.parent().unwrap_or(path);
                    if w.watch(dir, RecursiveMode::NonRecursive).is_ok() {
                        Some(Box::new(w))
                    } else {
                        error!("pager: FileTailer could not watch {}", dir.display());
                        None
                    }
                }
                Err(e) => {
                    error!("pager: FileTailer could not start watcher: {e}");
                    None
                }
            };

        Ok(Self {
            path: path.to_path_buf(),
            file,
            offset,
            stop,
            rx,
            _watcher,
        })
    }
}

impl Read for FileTailer {
    fn read(
        &mut self,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        loop {
            if self.stop.load(Ordering::Relaxed) {
                return Ok(0);
            }
            // Detect truncation / rotation.
            let len = self.file.metadata()?.len();
            if len < self.offset {
                self.file = File::open(&self.path)?;
                self.offset = 0;
            }
            let n = self.file.read(buf)?;
            if n > 0 {
                self.offset += n as u64;
                return Ok(n);
            }
            // Block until a FS event wakes us or the timeout elapses.
            // The 10 ms timeout is a fallback for the watcher-absent case.
            let _ = self.rx.recv_timeout(Duration::from_millis(10));
        }
    }
}

/// A read adapter that yields only lines in the 1-based range `[start, end]`.
/// `end = None` yields everything from `start` onward.
struct LineWindowReader<R: BufRead> {
    inner: R,
    current_line: usize,
    start: usize,
    end: Option<usize>,
    pending: Vec<u8>,
}

impl<R: BufRead> LineWindowReader<R> {
    fn new(
        inner: R,
        start: usize,
        end: Option<usize>,
    ) -> Self {
        Self {
            inner,
            current_line: 1,
            start: start.max(1),
            end,
            pending: Vec::new(),
        }
    }
}

impl<R: BufRead> Read for LineWindowReader<R> {
    fn read(
        &mut self,
        buf: &mut [u8],
    ) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            // Flush pending bytes from the last accepted line.
            if !self.pending.is_empty() {
                let n = self.pending.len().min(buf.len());
                buf[..n].copy_from_slice(&self.pending[..n]);
                self.pending.drain(..n);
                return Ok(n);
            }
            // Signal EOF when the window is exhausted.
            if let Some(end) = self.end {
                if self.current_line > end {
                    return Ok(0);
                }
            }
            // Read the next source line.
            let mut line = String::new();
            match self.inner.read_line(&mut line) {
                Ok(0) => return Ok(0),
                Ok(_) => {
                    if self.current_line >= self.start {
                        self.pending = line.into_bytes();
                    }
                    self.current_line += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Returns `true` when `path` has unstaged changes tracked by git.
/// Returns `false` on any error (git absent, not a git repo, etc.).
pub fn has_git_changes(path: &Path) -> bool {
    let dir = path.parent().unwrap_or(path);
    Command::new("git")
        .args(["-C"])
        .arg(dir)
        .args(["diff", "--name-only", "--"])
        .arg(path)
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

/// Count the number of lines in `path` by reading through it once.
pub fn count_lines(path: &Path) -> io::Result<usize> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut count = 0usize;
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => count += 1,
            Err(e) => return Err(e),
        }
    }
    Ok(count)
}

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
    // Include bat's exit status when no command child exists so that missing
    // files propagate errors.
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
/// `alt-h for help, q to quit`). Loaded through [`pager_cfg`], which works in
/// subtool processes too (no `GLOBAL::init` needed).
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
/// If `stop` is `Some`, id-3 sets it on pager exit to cancel a running
/// [`FileTailer`].
fn configure_hooks(
    pager: &Pager,
    cmd_child: &Arc<Mutex<Option<Child>>>,
    bat_child: &Arc<Mutex<Option<Child>>>,
    stop: Option<Arc<AtomicBool>>,
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
    if let Some(stop) = stop {
        let _ = pager.add_hook(
            Hook::PostPagerExit,
            3,
            Box::new(move |_| {
                stop.store(true, Ordering::Relaxed);
            }),
        );
    }
}

/// Spawn `bat <opts> [path]` with piped stdin/stdout. With a path, bat opens
/// the file itself (language detection works) and stdin stays unused; without
/// one, the caller feeds the content on a writer thread so a failed spawn can
/// fall back to the raw stream.
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

/// Wrap `raw` in a [`LineWindowReader`] when `range` is set and stdout is not
/// a TTY (or `force_tty` is false). Otherwise returns `raw` unchanged.
fn apply_fallback_window(
    raw: Box<dyn Read + Send>,
    range: Option<(usize, Option<usize>)>,
    force_tty: bool,
) -> Box<dyn Read + Send> {
    if !force_tty && !atty::is(atty::Stream::Stdout) {
        if let Some((start, end)) = range {
            return Box::new(LineWindowReader::new(BufReader::new(raw), start, end));
        }
    }
    raw
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
    page_inner(
        Ok(Box::new(stdout)),
        Some(child),
        true,
        bat,
        PagerOpts::default(),
    )
}

/// Render any reader (no child). `force_tty=false` for the subtool paths:
/// stdout is probed — a terminal gets interactive minus, a pipe or file is
/// passed straight through without paging. Returns whether the stream was
/// non-empty.
pub fn page_reader<R: Read + Send + 'static>(
    r: R,
    force_tty: bool,
    bat: Option<Vec<String>>,
    opts: PagerOpts,
) -> io::Result<bool> {
    page_inner(Ok(Box::new(r)), None, force_tty, bat, opts)
}

/// Render a file to the current process's stdout (the subtool's stdout when
/// run from the lessfilter executor). Bat args come from the subtool env logic.
/// The path goes to bat directly (no first-line gate — the file is known to
/// exist). Returns whether the file rendered; `Err` when it cannot be opened
/// and `Ok(false)` when bat fails to render it.
pub fn render_text(
    path: &Path,
    bat: Option<Vec<String>>,
    opts: PagerOpts,
) -> io::Result<bool> {
    page_inner(Err(path.to_path_buf()), None, false, bat, opts)
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
    opts: PagerOpts,
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
            if let Some(bat_opts) = bat.filter(|_| has("bat")) {
                match spawn_bat(bat_opts, None) {
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
                        apply_fallback_window(raw, opts.fallback_range, force_tty)
                    }
                }
            } else {
                apply_fallback_window(raw, opts.fallback_range, force_tty)
            }
        }
        Err(path) => match bat.filter(|_| has("bat")) {
            Some(bat_opts) => match spawn_bat(bat_opts, Some(&path)) {
                Ok((child, _stdin, stdout)) => {
                    bat_child = Some(child);
                    Box::new(stdout)
                }
                Err(e) => {
                    error!("pager: failed spawning bat: {e}");
                    // bat failed to spawn: open the file in-process and apply
                    // the fallback window if requested.
                    let raw = open_source(&path)?;
                    apply_fallback_window(raw, opts.fallback_range, force_tty)
                }
            },
            None => {
                let raw = open_source(&path)?;
                apply_fallback_window(raw, opts.fallback_range, force_tty)
            }
        },
    };

    let cmd_child = Arc::new(Mutex::new(child));
    let bat_child = Arc::new(Mutex::new(bat_child));

    if force_tty || atty::is(atty::Stream::Stdout) {
        // Interactive minus: on `/dev/tty` when forced (execute paths), else on
        // the default stdout sink (the caller's stdout is a terminal, e.g.
        // `:tool pager` run from a shell).
        page_tty(
            feed,
            &cmd_child,
            &bat_child,
            force_tty,
            opts.scroll_to,
            opts.follow,
            opts.stop,
        )
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
///
/// If `follow` is true, enables auto-scroll to the bottom as content arrives.
/// If `scroll_to` is set, jumps to that 0-indexed line after the first line is
/// pushed. If `stop` is set, the PostPagerExit hook stores `true` into it to
/// cancel a running [`FileTailer`].
fn page_tty(
    mut source: Box<dyn Read + Send>,
    cmd_child: &Arc<Mutex<Option<Child>>>,
    bat_child: &Arc<Mutex<Option<Child>>>,
    force_tty: bool,
    scroll_to: Option<usize>,
    follow: bool,
    stop: Option<Arc<AtomicBool>>,
) -> io::Result<bool> {
    let pager = Pager::new();
    configure_pager(&pager);
    if follow {
        let _ = pager.follow_output(true);
    }
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
    configure_hooks(&pager, cmd_child, bat_child, stop);

    let pager_for_thread = pager.clone();
    let pager_thread = thread::spawn(move || {
        let _ = minus::dynamic_paging(pager_for_thread);
    });

    let mut feed = BufReader::new(source);
    let mut line = String::new();
    let mut first_pushed = false;
    loop {
        line.clear();
        match feed.read_line(&mut line) {
            Ok(0) => break, // EOF: the command finished (or was killed)
            Ok(_) => {
                if pager.push_str(line.clone()).is_err() {
                    break; // pager quit; stop feeding
                }
                if !first_pushed {
                    first_pushed = true;
                    if let Some(target) = scroll_to {
                        let _ = pager.go_to_line(target);
                    }
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
