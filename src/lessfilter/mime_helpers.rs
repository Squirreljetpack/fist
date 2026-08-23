use std::{fs::File, io::Read, path::Path, str::FromStr};

use mime_guess::Mime;

use crate::lessfilter::InferMode;
use fist_types::FileCategory;

// todo: flesh out the kind using a mime -> fc helper
// wrapper cuz we can't add a param to Mime
#[derive(Default, Debug)]
pub struct Myme {
    // not sure if we want just a simple MimeString here or no
    pub mime: Option<Mime>,
    /// A file category whose method of determination is dependent on the configured [`infer_mode`]:
    /// [`InferMode::FileFormat`]: Attempt to match the file to a catalogue of known types by finding a compatible reader. The mime and category are defined in the catalogue.
    /// [`InferMode::Infer`]: None.
    /// [`InferMode::Guess`]: `None`.
    pub kind: Option<FileCategory>,
}

impl Myme {
    pub fn from_path(
        path: &Path,
        infer_mode: InferMode,
    ) -> Myme {
        // not sure if its faster to do this pre-check
        if path.is_dir() {
            return Myme {
                mime: Mime::from_str("directory/*").ok(),
                kind: None,
            };
        }

        #[cfg(feature = "infer")]
        let maybe_type = infer::get_from_path(path).ok().flatten();
        #[cfg(feature = "file-format")]
        let maybe_format = file_format::FileFormat::from_file(path).ok();

        // easier with https://github.com/rust-lang/rust/issues/51114
        let (mime, kind) = match infer_mode {
            #[cfg(feature = "file-format")]
            InferMode::FileFormat if maybe_format.is_some() => {
                let format = maybe_format.unwrap();
                (
                    format.media_type().parse().ok(),
                    Some(FileCategory::from_fileformat(format)),
                )
            }
            #[cfg(feature = "infer")]
            InferMode::Infer if maybe_type.is_some() => {
                (maybe_type.unwrap().mime_type().parse().ok(), None)
            }
            _ => {
                let guess = mime_guess::from_path(path);
                (guess.first(), None)
            }
        };

        Myme { mime, kind }
    }
}

pub fn detect_encoding(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;

    // Read at most 64 KB
    let mut buf = Vec::new();
    file.take(64 * 1024).read_to_end(&mut buf).ok()?;

    let result = charset_normalizer_rs::from_bytes(&buf, None).ok()?;
    let best = result.get_best()?;

    let enc = best.encoding().to_ascii_lowercase();

    Some(enc)
}

pub fn is_native(enc: &str) -> bool {
    enc.contains("utf-8")
        || enc.contains("unicode")
        || enc.contains("ascii")
        || enc.contains("iso-8859")
        || enc.contains("windows-125")
        || enc.contains("mac")
}
