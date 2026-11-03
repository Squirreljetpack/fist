//! Wayland file clipboard, served through `wlr-data-control`.
//!
//! Wayland has no clipboard owner either: a copy is a set of data sources, and
//! the compositor forwards each paste as a file-descriptor pipe request to
//! whoever holds them. Plain `wl_data_device` only works for a client with a
//! focused surface, so fist goes through the `wlr-data-control` protocol as
//! `wl-copy` does.
//!
//! `wl-clipboard-rs` serves in a detached thread: `copy_multi` returns once the
//! clipboard has been handed over, and that thread keeps answering paste
//! requests until another application copies something. It dies with the
//! process, so — as with the X11 owner — the data is gone once fist quits
//! unless a clipboard manager keeps its own copy.

use wl_clipboard_rs::copy::{MimeSource, MimeType, Options, Source};

use super::{Clip, CopyMode, Error};

/// Take the clipboard and offer `clip` under every flavour.
pub fn set(clip: &Clip) -> Result<(), Error> {
    let mut options = Options::new();
    // The uri-list is not text: without this the crate re-advertises it under
    // UTF8_STRING/STRING, and a terminal would paste the URI list for ctrl-v.
    options.omit_additional_text_mime_types(true);

    let mut sources = vec![
        bytes(&super::uri_list(&clip.paths), "text/uri-list"),
        bytes(
            &super::gnome_copied(&clip.paths, clip.mode),
            "x-special/gnome-copied-files",
        ),
        // KDE's cut marker; the GNOME flavour above carries the same meaning
        bytes(
            if clip.mode == CopyMode::Cut { "1" } else { "0" },
            "application/x-kde-cutselection",
        ),
        bytes(&clip.text, "text/plain;charset=utf-8"),
        bytes(&clip.text, "text/plain"),
        bytes(&clip.text, "UTF8_STRING"),
        bytes(&clip.text, "TEXT"),
    ];
    if clip.text.is_ascii() {
        sources.push(bytes(&clip.text, "STRING"));
    }
    if let Some(png) = &clip.png {
        sources.push(MimeSource {
            source: Source::Bytes(png.as_slice().into()),
            mime_type: MimeType::Specific("image/png".into()),
        });
    }
    options
        .copy_multi(sources)
        .map_err(|e| Error(e.to_string()))
}

/// One flavour served from memory.
fn bytes(
    data: &str,
    mime_type: &str,
) -> MimeSource {
    MimeSource {
        source: Source::Bytes(data.as_bytes().into()),
        mime_type: MimeType::Specific(mime_type.into()),
    }
}
