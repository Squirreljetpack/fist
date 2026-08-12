use arboard::{Clipboard, ImageData};
use cba::bait::{OptionExt, ResultExt};
use image::ImageReader;
use matchmaker::message::RenderCommand;
use ratatui::text::Span;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::osc52;
use crate::run::item::short_display;
use crate::run::state::{GLOBAL, TASKS, TOAST};
use crate::utils::text::ToastStyle;

/// The clipboard slot: `Some` = local arboard backend, `None` = OSC52.
///
/// The backend is chosen once in [`init`] and is authoritative: with
/// `[tui].osc52` enabled arboard is never initialized; with it disabled a
/// failed arboard initialization is logged and the slot stays `None`, so copy
/// dispatch falls back to OSC52 (the only remaining route).
pub type FsClipboard = Option<Clipboard>;

pub static CLIPBOARD: Mutex<FsClipboard> = Mutex::new(None);
pub static CLIPBOARD_SLEEP_MS: AtomicU64 = AtomicU64::new(20);

pub fn init(
    cb_sleep: u64,
    osc52: bool,
) {
    let err_prefix = "Failed to initialize clipboard";
    CLIPBOARD_SLEEP_MS.store(cb_sleep, Ordering::Release);
    if let Ok(mut cb) = CLIPBOARD.lock().ok().elog(err_prefix) {
        *cb = if osc52 {
            None
        } else {
            Clipboard::new().prefix(err_prefix)._elog()
        };
    }
}

/// Serialize paths into one OSC52 payload: newline-delimited, no final
/// newline.
pub fn serialize_paths(paths: &[PathBuf]) -> String {
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

/// Report a completed OSC52 write: success toast (optionally) plus a redraw,
/// or the existing error-toast path on failure.
fn report_osc52(
    result: std::io::Result<()>,
    spans: Vec<Span<'static>>,
    toast: bool,
) {
    match result {
        Ok(()) => {
            if toast {
                TOAST::push(ToastStyle::Success, "Copied: ", spans);
            }
            // the sequence bypassed the TUI stream: repaint
            GLOBAL::send_mm(RenderCommand::Redraw);
        }
        Err(e) => {
            log::error!("OSC52 clipboard write failed: {e}");
            TOAST::push(ToastStyle::Error, "Failed to copy: ", spans);
        }
    }
}

fn item_delay() -> Duration {
    Duration::from_millis(CLIPBOARD_SLEEP_MS.load(Ordering::Relaxed))
}

pub fn copy_texts(
    texts: Vec<String>,
    toast: bool,
) {
    if texts.is_empty() {
        return;
    }
    TASKS::spawn_blocking(move || {
        let mut guard = CLIPBOARD.lock().unwrap();
        match guard.as_mut() {
            Some(cb) => {
                let mut success = Vec::new();
                let mut failed = Vec::new();
                for (i, text) in texts.iter().enumerate() {
                    match cb.set_text(text.clone()) {
                        Ok(_) => success.push(Span::from(text.clone())),
                        Err(_) => failed.push(Span::from(text.clone())),
                    }
                    if i + 1 < texts.len() {
                        std::thread::sleep(item_delay());
                    }
                }
                drop(guard);
                if !failed.is_empty() {
                    TOAST::push(ToastStyle::Error, "Failed to copy: ", failed);
                }
                if toast && !success.is_empty() {
                    TOAST::push(ToastStyle::Success, "Copied: ", success);
                }
            }
            None => {
                let spans = texts.iter().map(|t| text_summary(t)).collect();
                let result = osc52::write_osc52(&texts.join("\n"));
                drop(guard);
                report_osc52(result, spans, toast);
            }
        }
    });
}

pub fn copy_paths_as_text(
    paths: Vec<PathBuf>,
    toast: bool,
) {
    if paths.is_empty() {
        return;
    }
    TASKS::spawn_blocking(move || {
        let spans: Vec<Span<'static>> = paths.iter().map(|p| short_display(p)).collect();
        let mut guard = CLIPBOARD.lock().unwrap();
        match guard.as_mut() {
            Some(cb) => {
                let mut success = Vec::new();
                let mut failed = Vec::new();
                for (i, path) in paths.iter().enumerate() {
                    match cb.set_text(path.to_string_lossy()) {
                        Ok(_) => success.push(short_display(path)),
                        Err(_) => failed.push(short_display(path)),
                    }
                    if i + 1 < paths.len() {
                        std::thread::sleep(item_delay());
                    }
                }
                drop(guard);
                if !failed.is_empty() {
                    TOAST::push(ToastStyle::Error, "Failed to copy: ", failed);
                }
                if toast && !success.is_empty() {
                    TOAST::push(ToastStyle::Success, "Copied: ", success);
                }
            }
            None => {
                let result = osc52::write_osc52(&serialize_paths(&paths));
                drop(guard);
                report_osc52(result, spans, toast);
            }
        }
    });
}

pub fn copy_files(
    paths: Vec<PathBuf>,
    toast: bool,
) {
    if paths.is_empty() {
        return;
    }
    TASKS::spawn_blocking(move || {
        let spans: Vec<Span<'static>> = paths.iter().map(|p| short_display(p)).collect();
        let mut guard = CLIPBOARD.lock().unwrap();
        match guard.as_mut() {
            Some(cb) => {
                let mut success = Vec::new();
                let mut failed = Vec::new();

                for (i, path) in paths.iter().enumerate() {
                    let image_data_opt = if let Some(mime) = mime_guess::from_path(path).first() {
                        if mime.type_() == mime_guess::mime::IMAGE {
                            ImageReader::open(path)
                                .ok()
                                .and_then(|reader| reader.decode().ok())
                                .map(|img| {
                                    let rgba = img.into_rgba8();
                                    let (w, h) = rgba.dimensions();
                                    ImageData {
                                        width: w as usize,
                                        height: h as usize,
                                        bytes: Cow::Owned(rgba.into_raw()),
                                    }
                                })
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let result = match image_data_opt {
                        Some(data) => cb.set_image(data),
                        None => cb.set_text(path.to_string_lossy()),
                    };

                    match result {
                        Ok(_) => success.push(short_display(path)),
                        Err(_) => failed.push(short_display(path)),
                    }

                    if i + 1 < paths.len() {
                        std::thread::sleep(item_delay());
                    }
                }
                drop(guard);
                if !failed.is_empty() {
                    TOAST::push(ToastStyle::Error, "Failed to copy: ", failed);
                }
                if toast && !success.is_empty() {
                    TOAST::push(ToastStyle::Success, "Copied: ", success);
                }
            }
            None => {
                // OSC52 is text-only: send the paths, not image bytes
                let result = osc52::write_osc52(&serialize_paths(&paths));
                drop(guard);
                report_osc52(result, spans, toast);
            }
        }
    });
}

/// Copy one complete text payload (command output) through the initialized
/// backend. Owns the whole lock boundary: lock `CLIPBOARD`, write, unlock,
/// then toast/redraw.
pub fn copy_text(
    text: String,
    toast: bool,
) {
    if text.is_empty() {
        return;
    }
    TASKS::spawn_blocking(move || {
        let span = text_summary(&text);
        let mut guard = CLIPBOARD.lock().unwrap();
        match guard.as_mut() {
            Some(cb) => {
                let result = cb.set_text(text);
                drop(guard);
                match result {
                    Ok(_) => {
                        if toast {
                            TOAST::push(ToastStyle::Success, "Copied: ", [span]);
                        }
                    }
                    Err(e) => {
                        log::error!("Clipboard set_text failed: {e}");
                        TOAST::push(ToastStyle::Error, "Failed to copy: ", [span]);
                    }
                }
            }
            None => {
                let result = osc52::write_osc52(&text);
                drop(guard);
                report_osc52(result, vec![span], toast);
            }
        }
    });
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
        assert_eq!(
            serialize_paths(&[PathBuf::from("/tmp/a")]),
            "/tmp/a"
        );
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
