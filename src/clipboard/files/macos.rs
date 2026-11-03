//! macOS file clipboard: pasteboard items holding file URLs.
//!
//! `NSPasteboard` takes one item per file, each carrying a `file://` URL as
//! `public.file-url` — what Finder writes and reads. The first item also
//! offers the text and image flavours, so a terminal or an image editor still
//! pastes something sensible. macOS has no cut marker on the pasteboard:
//! whether files are copied or moved is decided by the destination, so a cut
//! is served as a plain file copy.

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSPasteboard, NSPasteboardItem, NSPasteboardTypeFileURL, NSPasteboardTypePNG,
    NSPasteboardTypeString, NSPasteboardWriting,
};
use objc2_foundation::{NSArray, NSData, NSString, NSURL};

use super::{Clip, Error};

/// Put the files, their path text and any image on the general pasteboard.
pub fn set(clip: &Clip) -> Result<(), Error> {
    autoreleasepool(|_| {
        let mut items: Vec<Retained<NSPasteboardItem>> =
            Vec::with_capacity(clip.paths.len().max(1));
        for path in &clip.paths {
            let path = NSString::from_str(&path.to_string_lossy());
            let Some(url) = NSURL::fileURLWithPath(&path).absoluteString() else {
                continue;
            };
            let item = NSPasteboardItem::new();
            item.setString_forType(&url, unsafe { NSPasteboardTypeFileURL });
            items.push(item);
        }
        if let Some(first) = items.first() {
            first.setString_forType(&NSString::from_str(&clip.text), unsafe {
                NSPasteboardTypeString
            });
            if let Some(png) = &clip.png {
                first.setData_forType(&NSData::with_bytes(png), unsafe { NSPasteboardTypePNG });
            }
        } else {
            let item = NSPasteboardItem::new();
            item.setString_forType(&NSString::from_str(&clip.text), unsafe {
                NSPasteboardTypeString
            });
            if let Some(png) = &clip.png {
                item.setData_forType(&NSData::with_bytes(png), unsafe { NSPasteboardTypePNG });
            }
            items.push(item);
        }

        let writers: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = items
            .into_iter()
            .map(ProtocolObject::from_retained)
            .collect();
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        if !pasteboard.writeObjects(&NSArray::from_retained_slice(&writers)) {
            return Err(Error("pasteboard rejected the file list".into()));
        }
        Ok(())
    })
}
