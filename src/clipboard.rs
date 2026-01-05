use arboard::{Clipboard, ImageData};
use image::ImageReader;
use ratatui::text::Span;
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::run::globals::TOAST;
use crate::run::item::short_display;
use crate::utils::text::ToastStyle;

pub static CLIPBOARD: Mutex<Option<Clipboard>> = Mutex::new(None);
pub static CLIPBOARD_SLEEP_MS: AtomicU64 = AtomicU64::new(20);

pub fn copy_texts(
    texts: Vec<String>,
    toast: bool,
) {
    tokio::spawn(async move {
        let mut success = Vec::new();
        let mut failed = Vec::new();

        for (i, text) in texts.into_iter().enumerate() {
            let result = {
                let mut guard = CLIPBOARD.lock().unwrap();
                if let Some(cb) = guard.as_mut() {
                    cb.set_text(text.clone())
                } else {
                    return;
                }
            };

            match result {
                Ok(_) => success.push(Span::from(text)),
                Err(e) => failed.push(Span::from(text)),
            }
        }

        if !failed.is_empty() {
            TOAST::push(ToastStyle::Error, "Failed to copy: ", failed);
        }
        if toast && !success.is_empty() {
            TOAST::push(ToastStyle::Success, "Copied: ", success);
        }

        tokio::time::sleep(Duration::from_millis(
            CLIPBOARD_SLEEP_MS.load(Ordering::Relaxed),
        ))
        .await;
    });
}

pub fn copy_paths_as_text(
    texts: Vec<PathBuf>,
    toast: bool,
) {
    tokio::spawn(async move {
        let mut success = Vec::new();
        let mut failed = Vec::new();

        for (i, text) in texts.into_iter().enumerate() {
            let result = {
                let mut guard = CLIPBOARD.lock().unwrap();
                if let Some(cb) = guard.as_mut() {
                    cb.set_text(text.to_string_lossy())
                } else {
                    return;
                }
            };

            match result {
                Ok(_) => success.push(short_display(&text)),
                Err(e) => failed.push(short_display(&text)),
            }

            tokio::time::sleep(Duration::from_millis(
                CLIPBOARD_SLEEP_MS.load(Ordering::Relaxed),
            ))
            .await;
        }

        if !failed.is_empty() {
            TOAST::push(ToastStyle::Error, "Failed to copy: ", failed);
        }
        if toast && !success.is_empty() {
            TOAST::push(ToastStyle::Success, "Copied: ", success);
        }
    });
}

pub fn copy_files(
    paths: Vec<PathBuf>,
    toast: bool,
) {
    tokio::spawn(async move {
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

            let result = {
                let mut guard = CLIPBOARD.lock().unwrap();
                if let Some(cb) = guard.as_mut() {
                    match image_data_opt {
                        Some(data) => cb.set_image(data),
                        None => cb.set_text(path.to_string_lossy()),
                    }
                } else {
                    return;
                }
            };

            match result {
                Ok(_) => success.push(short_display(path)),
                Err(_) => failed.push(short_display(path)),
            }

            tokio::time::sleep(Duration::from_millis(
                CLIPBOARD_SLEEP_MS.load(Ordering::Relaxed),
            ))
            .await;
        }

        if !failed.is_empty() {
            TOAST::push(ToastStyle::Error, "Failed to copy: ", failed);
        }
        if toast && !success.is_empty() {
            TOAST::push(ToastStyle::Success, "Copied: ", success);
        }
    });
}
