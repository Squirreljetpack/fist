//! Windows file clipboard: `CF_HDROP` plus text, bitmap, and cut flavours.
//!
//! `CF_HDROP` is a `DROPFILES` header followed by a double-NUL-terminated list
//! of UTF-16 paths — what Explorer writes for a copied selection. It shares the
//! clipboard with `Preferred DropEffect` (to distinguish cut/move from copy),
//! `CF_UNICODETEXT`, and `CF_DIB` / `PNG` flavours. All of them are written
//! inside one opened clipboard session.

use clipboard_win::formats::{CF_DIB, CF_UNICODETEXT};
use clipboard_win::{Clipboard, raw};

use super::{Clip, CopyMode, Error};

/// Put the file list, its path text and any image on the clipboard.
pub fn set(clip: &Clip) -> Result<(), Error> {
    let _guard = Clipboard::new_attempts(10).map_err(err)?;
    write(clip)
}

fn write(clip: &Clip) -> Result<(), Error> {
    raw::empty().map_err(err)?;
    let paths = clip
        .paths
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    raw::set_file_list(&paths).map_err(err)?;
    if let Some(format) = clipboard_win::register_format("Preferred DropEffect") {
        let effect: u32 = match clip.mode {
            CopyMode::Copy => 1, // DROPEFFECT_COPY
            CopyMode::Cut => 2,  // DROPEFFECT_MOVE
        };
        let _ = raw::set_without_clear(format.get(), &effect.to_le_bytes());
    }
    if let Some(png) = &clip.png {
        if let Some(format) = clipboard_win::register_format("PNG") {
            let _ = raw::set_without_clear(format.get(), png);
        }
        if let Some(dib) = dib(png) {
            let _ = raw::set_without_clear(CF_DIB, &dib);
        }
    }
    raw::set_without_clear(CF_UNICODETEXT, &utf16(&clip.text)).map_err(err)?;
    Ok(())
}

fn err(e: clipboard_win::ErrorCode) -> Error {
    Error(e.to_string())
}

/// A `CF_UNICODETEXT` payload is one NUL-terminated UTF-16 string.
fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// `CF_DIB` bytes for a PNG payload: a `BITMAPINFOHEADER` followed by bottom-up
/// BGRA rows.
fn dib(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    let mut dib = Vec::with_capacity(40 + (width * height * 4) as usize);
    dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&(height as i32).to_le_bytes()); // biHeight, bottom-up
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    dib.extend_from_slice(&(width * height * 4).to_le_bytes()); // biSizeImage
    dib.extend_from_slice(&[0; 16]); // resolution, palette sizes
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);
            dib.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
    }
    Some(dib)
}
