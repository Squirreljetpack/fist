//! File references on the system clipboard: "these files", not their text.
//!
//! A file selection is not a clipboard scalar. Every platform has its own
//! structure for it and none of them is arboard's text or image payload, so
//! this module owns the file half of the clipboard:
//!
//! | platform | mechanism |
//! | --- | --- |
//! | X11 | a serving selection owner (`x11`) |
//! | Wayland | `wl_data_device` sources served by `wl-clipboard-rs` (`wayland`) |
//! | Windows | `CF_HDROP` (`windows`) |
//! | macOS | pasteboard items holding file URLs (`macos`) |
//!
//! Every backend also offers a plain-text flavour, and a PNG flavour when the
//! selection is one image, so pasting into a terminal or an image editor keeps
//! working. OSC 52 is text-only and never used here.
//!
//! Linux picks its backend at runtime: a Wayland session whose compositor
//! grants `wlr-data-control` is served by `wl-clipboard-rs`, everything else by
//! an X11 selection owner. Both need a live process on the other end, so fist
//! keeps a serving thread (or child) until another application takes the
//! selection over: quitting fist empties the clipboard unless a clipboard
//! manager re-owns it.

use std::io::Cursor;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

/// Whether the payload means "copy these files" or "move (cut) them".
/// Supported on Linux (GNOME/KDE) and Windows (Preferred DropEffect); macOS
/// pastes a copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CopyMode {
    Copy,
    Cut,
}

/// One prepared file copy: the paths plus every flavour the backends offer.
pub struct Clip {
    pub paths: Vec<PathBuf>,
    /// Newline-separated absolute paths, for terminals.
    pub text: String,
    /// PNG bytes, when the selection is one image.
    pub png: Option<Vec<u8>>,
    pub mode: CopyMode,
}

impl Clip {
    /// Build the payload for `paths`. Called on the task pool, since decoding
    /// and re-encoding an image is not free.
    pub fn new(
        paths: Vec<PathBuf>,
        mode: CopyMode,
    ) -> Self {
        let text = paths
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        let png = single_image(&paths).and_then(png_bytes);
        Self {
            paths,
            text,
            png,
            mode,
        }
    }
}

/// A file-copy failure, already worded for the log and the toast.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);

/// Put `clip` on the system clipboard, replacing whatever was there.
pub fn set(clip: Clip) -> Result<(), Error> {
    #[cfg(target_os = "linux")]
    return linux_set(clip);
    #[cfg(windows)]
    return windows::set(&clip);
    #[cfg(target_os = "macos")]
    return macos::set(&clip);
}

/// Wayland first when the session is Wayland: XWayland usually still answers,
/// and losing the copy entirely is worse than serving it through X.
#[cfg(target_os = "linux")]
fn linux_set(clip: Clip) -> Result<(), Error> {
    if on_wayland() {
        match wayland::set(&clip) {
            Ok(()) => return Ok(()),
            Err(e) => log::warn!("Wayland clipboard unavailable: {e}"),
        }
    }
    x11::set(clip)
}

/// The selected path when the selection is exactly one image file. A clipboard
/// holds one image, so an ambiguous selection offers none.
pub fn single_image(paths: &[PathBuf]) -> Option<&Path> {
    let [path] = paths else {
        return None;
    };
    let mime = mime_guess::from_path(path).first()?;
    (mime.type_() == mime_guess::mime::IMAGE).then_some(path.as_path())
}

/// `file://` URI for `path`, percent-encoding every byte outside the RFC 3986
/// path set.
#[cfg(target_os = "linux")]
fn file_uri(path: &Path) -> String {
    use std::fmt::Write as _;

    let mut uri = String::with_capacity(path.as_os_str().len() + 8);
    uri.push_str("file://");
    for &byte in path.as_os_str().as_encoded_bytes() {
        match byte {
            b'/'
            | b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'='
            | b':'
            | b'@' => {
                uri.push(byte as char);
            }
            _ => {
                let _ = write!(uri, "%{byte:02X}");
            }
        }
    }
    uri
}

/// `text/uri-list`: CRLF-separated `file://` URIs, no trailing CRLF.
#[cfg(target_os = "linux")]
pub(super) fn uri_list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| file_uri(p))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// `x-special/gnome-copied-files`: the mode line, then newline-separated URIs.
/// This tells GNOME/GTK paste targets whether to move or duplicate.
#[cfg(target_os = "linux")]
pub(super) fn gnome_copied(
    paths: &[PathBuf],
    mode: CopyMode,
) -> String {
    let head = match mode {
        CopyMode::Copy => "copy",
        CopyMode::Cut => "cut",
    };
    let uris = paths
        .iter()
        .map(|p| file_uri(p))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{head}\n{uris}")
}

#[cfg(target_os = "linux")]
fn on_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

/// PNG bytes for `path`; the flavour is simply absent from builds without an
/// image encoder.
#[cfg(any(feature = "image-defaults", feature = "image-all"))]
fn png_bytes(path: &Path) -> Option<Vec<u8>> {
    let img = image::ImageReader::open(path).ok()?.decode().ok()?;
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some(buf.into_inner())
}

#[cfg(not(any(feature = "image-defaults", feature = "image-all")))]
fn png_bytes(_path: &Path) -> Option<Vec<u8>> {
    None
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn file_uri_encodes_specials() {
        assert_eq!(file_uri(Path::new("/tmp/a b")), "file:///tmp/a%20b");
        assert_eq!(file_uri(Path::new("/tmp/a%20b")), "file:///tmp/a%2520b");
        assert_eq!(
            file_uri(Path::new("/tmp/ünïcode")),
            "file:///tmp/%C3%BCn%C3%AFcode"
        );
        assert_eq!(file_uri(Path::new("/tmp/a?b#c")), "file:///tmp/a%3Fb%23c");
    }

    #[test]
    fn uri_list_separates_with_crlf() {
        let paths = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];
        assert_eq!(uri_list(&paths), "file:///tmp/a\r\nfile:///tmp/b");
    }

    #[test]
    fn gnome_copied_marker_follows_mode() {
        let paths = vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")];
        assert_eq!(
            gnome_copied(&paths, CopyMode::Copy),
            "copy\nfile:///tmp/a\nfile:///tmp/b"
        );
        assert_eq!(
            gnome_copied(&paths, CopyMode::Cut),
            "cut\nfile:///tmp/a\nfile:///tmp/b"
        );
    }

    #[test]
    fn single_image_only_for_single_image_selection() {
        let image = vec![PathBuf::from("/tmp/a.png")];
        assert_eq!(single_image(&image), Some(Path::new("/tmp/a.png")));
        assert_eq!(single_image(&[PathBuf::from("/tmp/a.txt")]), None);
        assert_eq!(
            single_image(&[PathBuf::from("/tmp/a.png"), PathBuf::from("/tmp/b.png")]),
            None
        );
    }
}
