//! The clipboard model: one delivery path for text, another for files.
//!
//! [`CLIPBOARD`] holds the single [`FsClipboard`], initialized once at startup
//! ([`init`]). Copies arrive as a [`Payload`], which says what the clipboard
//! should afterwards contain:
//!
//! - Text goes through arboard, the local handle.
//! - File references go through [`files`], which speaks each platform's own
//!   structure (`text/uri-list` and `x-special/gnome-copied-files` on Linux,
//!   `CF_HDROP` on Windows, pasteboard items on macOS), alongside the path text
//!   and, for one image file, its bitmap. arboard cannot express "these files",
//!   so file copies bypass it entirely.
//!
//! `tui.osc52 = true` replaces the arboard handle with OSC 52, which writes the
//! text straight to the terminal — what makes copying work over ssh, and why
//! that mode never touches the machine's own clipboard. OSC 52 carries text
//! only, so in that mode a file copy degrades to the paths.
//!
//! The pager is the one caller outside the task pool: it copies from its own
//! thread ([`copy_from_pager`]), so that write is synchronous and logged
//! instead of toasted.

use arboard::Clipboard;
use cba::bait::{OptionExt, ResultExt};
use matchmaker::message::RenderCommand;
use ratatui::text::Span;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::run::item::short_display;
use crate::run::state::{GLOBAL, TASKS, TOAST, ToastFlags, ToastStyle};
use crate::utils::osc52;

pub mod files;

pub use files::CopyMode;

/// The local clipboard handle plus the settings every copy consults.
pub struct FsClipboard {
    /// The arboard handle. `None` when `tui.osc52` routes text to the terminal
    /// instead.
    pub arboard: Option<Clipboard>,
    /// Whether copied text keeps its trailing newline
    /// (`tui.copy_trailing_newline`).
    pub copy_trailing_newline: bool,
}

pub static CLIPBOARD: Mutex<Option<FsClipboard>> = Mutex::new(None);
static WRITE_LOCK: Mutex<()> = Mutex::new(());
static SEQ: AtomicU64 = AtomicU64::new(0);

/// Create the process-global clipboard handle. `use_osc52` skips arboard, so
/// every later text copy is written as an OSC 52 sequence.
pub fn init(
    use_osc52: bool,
    copy_trailing_newline: bool,
) {
    let err_prefix = "Failed to initialize clipboard";
    if let Ok(mut cb) = CLIPBOARD.lock().ok().elog(err_prefix) {
        let arboard = if use_osc52 {
            None
        } else {
            Clipboard::new().prefix(err_prefix)._elog()
        };
        *cb = Some(FsClipboard {
            arboard,
            copy_trailing_newline,
        });
    }
}

/// What a copy of the selected paths should leave on the clipboard.
#[derive(Clone, Copy)]
pub enum Payload {
    /// The paths as newline-separated text.
    Text,
    /// File references, marked as a copy or as a cut for the paste target.
    Files(CopyMode),
}

/// Which clipboard a completed copy reached.
enum Written {
    /// The machine's own clipboard: arboard, or a native file clipboard.
    Local,
    /// The terminal, via OSC 52: the UI stream was bypassed.
    Osc52,
}

/// Copy `paths` to the clipboard in `payload` form. Building a bitmap or
/// waiting on a clipboard server must not block the event loop, so the work
/// runs on the task pool.
pub fn copy_paths(
    paths: Vec<PathBuf>,
    payload: Payload,
    toast: bool,
) {
    if paths.is_empty() {
        return;
    }
    let seq = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    TASKS::spawn_blocking("clipboard copy paths", move || {
        let spans: Vec<Span<'static>> = paths.iter().map(|p| short_display(p)).collect();
        let text = serialize_paths(&paths);
        let clip = match payload {
            Payload::Files(mode) => Some(files::Clip::new(paths, mode)),
            Payload::Text => None,
        };
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if seq < SEQ.load(Ordering::Relaxed) {
            return;
        }
        let result = match clip {
            None => write(text),
            // OSC 52 has no file references: send the paths instead
            Some(_) if !local_active() => write(text),
            Some(clip) => files::set(clip)
                .map(|()| Written::Local)
                .map_err(|e| e.to_string()),
        };
        report(result, spans, toast);
    });
}

/// Copy one complete text payload (command output) through the active backend.
pub fn copy_text(
    text: String,
    toast: bool,
) {
    let seq = SEQ.fetch_add(1, Ordering::Relaxed) + 1;
    TASKS::spawn_blocking("clipboard copy text", move || {
        let span = text_summary(&text);
        let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if seq < SEQ.load(Ordering::Relaxed) {
            return;
        }
        report(write(text), vec![span], toast);
    });
}

/// Copy text from the pager through the active backend. The pager runs on its
/// own thread (outside the task pool), so the write happens synchronously and
/// failures are logged, not toasted.
pub fn copy_from_pager(text: String) {
    if text.is_empty() {
        return;
    }
    let _guard = WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = write(text) {
        log::error!("Pager clipboard copy failed: {e}");
    }
}

/// Read text from the active clipboard handle, if available.
pub fn get_text() -> Option<String> {
    let mut guard = lock();
    if let Some(fcb) = guard.as_mut() {
        fcb.arboard.as_mut()?.get_text().ok()
    } else {
        Clipboard::new().ok()?.get_text().ok()
    }
}

/// Write one text payload through the active backend. OSC 52 can only carry
/// text and applies the trailing-newline policy all the same.
fn write(mut text: String) -> Result<Written, String> {
    let mut guard = lock();
    let Some(fcb) = guard.as_mut() else {
        if let Ok(mut cb) = Clipboard::new() {
            return cb
                .set_text(text)
                .map(|()| Written::Local)
                .map_err(|e| e.to_string());
        }
        return osc52::write_osc52(&text)
            .map(|()| Written::Osc52)
            .map_err(|e| e.to_string());
    };
    apply_newline_policy(&mut text, fcb.copy_trailing_newline);
    match fcb.arboard.as_mut() {
        Some(cb) => cb
            .set_text(text)
            .map(|()| Written::Local)
            .map_err(|e| e.to_string()),
        None => osc52::write_osc52(&text)
            .map(|()| Written::Osc52)
            .map_err(|e| e.to_string()),
    }
}

/// Report a finished copy: the optional success toast, the repaint an OSC 52
/// write needs, or the error toast.
fn report(
    result: Result<Written, String>,
    spans: Vec<Span<'static>>,
    toast: bool,
) {
    match result {
        Ok(written) => {
            if toast {
                TOAST::push_with_flag(
                    ToastStyle::Success,
                    "Copied: ",
                    spans,
                    ToastFlags::SUSTAINED,
                );
            }
            if matches!(written, Written::Osc52) {
                // the sequence bypassed the TUI stream: repaint
                GLOBAL::send_mm(RenderCommand::Redraw);
            }
        }
        Err(e) => {
            log::error!("Clipboard write failed: {e}");
            TOAST::push(ToastStyle::Error, "Failed to copy: ", spans);
        }
    }
}

/// Whether copies can reach the local clipboard at all (arboard initialized and
/// not replaced by OSC 52).
fn local_active() -> bool {
    lock().as_ref().is_some_and(|fcb| fcb.arboard.is_some())
}

fn lock() -> MutexGuard<'static, Option<FsClipboard>> {
    CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner())
}

/// Serialize paths into one text payload: newline-delimited, no final newline.
fn serialize_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\n")
}

/// One-line, length-capped representation of copied text for toast display.
fn text_summary(text: &str) -> Span<'static> {
    const MAX_CHARS: usize = 32;
    let first = text.lines().next().unwrap_or_default();
    let mut s: String = first.chars().take(MAX_CHARS).collect();
    if first.chars().count() > MAX_CHARS {
        s.push('…');
    }
    Span::from(s)
}

fn apply_newline_policy(
    text: &mut String,
    copy_trailing_newline: bool,
) {
    if !copy_trailing_newline && text.ends_with('\n') {
        text.pop();

        if text.ends_with('\r') {
            text.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_paths_empty() {
        assert_eq!(serialize_paths(&[]), "");
    }

    #[test]
    fn serialize_paths_single_no_final_newline() {
        assert_eq!(serialize_paths(&[PathBuf::from("/tmp/a")]), "/tmp/a");
    }

    #[test]
    fn serialize_paths_multiple_newline_delimited() {
        let paths = vec![
            PathBuf::from("/tmp/a"),
            PathBuf::from("/tmp/b c"),
            PathBuf::from("/tmp/d"),
        ];
        assert_eq!(serialize_paths(&paths), "/tmp/a\n/tmp/b c\n/tmp/d");
    }

    #[test]
    fn text_summary_caps_first_line() {
        assert_eq!(text_summary("short").content, "short");
        assert_eq!(text_summary("line1\nline2").content, "line1");
        let long = "x".repeat(64);
        let summary = text_summary(&long);
        assert_eq!(summary.content.chars().count(), 33); // 32 + ellipsis
        assert!(summary.content.ends_with('…'));
    }
}
