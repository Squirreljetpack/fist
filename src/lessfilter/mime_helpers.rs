use std::{fs::File, io::Read, path::Path};

use charset_normalizer_rs::from_path;
use mime_guess::{Mime, mime};

// wrapper cuz we can't add a param to Mime


#[derive(Debug)]
pub struct Myme {
    pub mime: Mime,
    pub enc: Option<String>,
}

impl Myme {
    pub fn from_path(
        path: &Path,
        infer: bool,
    ) -> Myme {
        let mime: Mime = if path.is_dir() {
            "directory/*".parse().unwrap()
        } else if infer && let Some(kind) = infer::get_from_path(path).ok().flatten() {
            kind.mime_type()
                .parse()
                .unwrap_or(mime::APPLICATION_OCTET_STREAM)
        } else {
            let guess = mime_guess::from_path(path);
            guess.first().unwrap_or(mime::APPLICATION_OCTET_STREAM)
        };

        let enc = detect_charset(path);

        Myme { mime, enc }
    }
}

fn detect_charset(path: &Path) -> Option<String> {
    let result = from_path(path, None).ok()?;
    let best_guess = result.get_best()?;
    Some(best_guess.encoding().to_string())
}
