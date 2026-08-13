use arboard::{Clipboard, ImageData};
use cba::bait::{OptionExt, ResultExt};
use image::ImageReader;
use matchmaker::message::RenderCommand;
use ratatui::text::Span;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crate::run::item::short_display;
use crate::run::state::{GLOBAL, TASKS, TOAST};
use crate::utils::osc52;
use crate::utils::text::ToastStyle;

/// Holds the local arboard handle (if active) and configuration options.
pub struct FsClipboard {
    pub arboard: Option<Clipboard>,
    pub copy_trailing_newline: bool,
    pub item_delay: Duration,
}

pub static CLIPBOARD: Mutex<Option<FsClipboard>> = Mutex::new(None);

pub fn init(
    cb_sleep: u64,
    osc52: bool,
    copy_trailing_newline: bool,
) {
    let err_prefix = "Failed to initialize clipboard";
    if let Ok(mut cb) = CLIPBOARD.lock().ok().elog(err_prefix) {
        let arboard = if osc52 {
            None
        } else {
            Clipboard::new().prefix(err_prefix)._elog()
        };
        *cb = Some(FsClipboard {
            arboard,
            copy_trailing_newline,
            item_delay: Duration::from_millis(cb_sleep),
        });
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

pub fn apply_newline_policy(
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

pub fn copy_texts(
    texts: Vec<String>,
    toast: bool,
) {
    if texts.is_empty() {
        return;
    }
    TASKS::spawn_blocking(move || {
        let mut guard = CLIPBOARD.lock().unwrap();
        let item_delay = guard
            .as_ref()
            .map_or(Duration::from_millis(20), |fcb| fcb.item_delay);
        let arboard_opt = guard.as_mut().and_then(|fcb| fcb.arboard.as_mut());
        match arboard_opt {
            Some(cb) => {
                let mut success = Vec::new();
                let mut failed = Vec::new();
                for (i, text) in texts.iter().enumerate() {
                    match cb.set_text(text.clone()) {
                        Ok(_) => success.push(Span::from(text.clone())),
                        Err(_) => failed.push(Span::from(text.clone())),
                    }
                    if i + 1 < texts.len() {
                        std::thread::sleep(item_delay);
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
        let item_delay = guard
            .as_ref()
            .map_or(Duration::from_millis(20), |fcb| fcb.item_delay);
        let arboard_opt = guard.as_mut().and_then(|fcb| fcb.arboard.as_mut());
        match arboard_opt {
            Some(cb) => {
                let mut success = Vec::new();
                let mut failed = Vec::new();
                for (i, path) in paths.iter().enumerate() {
                    match cb.set_text(path.to_string_lossy()) {
                        Ok(_) => success.push(short_display(path)),
                        Err(_) => failed.push(short_display(path)),
                    }
                    if i + 1 < paths.len() {
                        std::thread::sleep(item_delay);
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
        let item_delay = guard
            .as_ref()
            .map_or(Duration::from_millis(20), |fcb| fcb.item_delay);
        let arboard_opt = guard.as_mut().and_then(|fcb| fcb.arboard.as_mut());
        match arboard_opt {
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
                        std::thread::sleep(item_delay);
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
    mut text: String,
    toast: bool,
) {
    TASKS::spawn_blocking(move || {
        let mut guard = CLIPBOARD.lock().unwrap();
        let copy_trailing_newline = guard
            .as_ref()
            .is_some_and(|fcb| fcb.copy_trailing_newline);
        apply_newline_policy(&mut text, copy_trailing_newline);
        if text.is_empty() {
            return;
        }
        let span = text_summary(&text);
        let arboard_opt = guard.as_mut().and_then(|fcb| fcb.arboard.as_mut());
        match arboard_opt {
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
