use super::{categories::FileCategory, filetypes::FileType};

/// Filetypes: [f, d, l, b, c, x, e, s, p].
///
/// Categories: [img, vid, aud, doc, tmp, src, conf, …].
///
/// Ext: '.*'
///
/// Groups: Configurable.
#[derive(Debug, Clone)]
pub enum FileTypeArg {
    Type(FileType),
    FileCategory(FileCategory),
    Ext(String),
    Group(String), // todo: custom groups in config
    /// Limit search to files with no extension
    NoExt,
}

impl std::str::FromStr for FileTypeArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self::NoExt);
        }
        let s_lower = s.to_lowercase();

        // extension if starts with "."
        if let Some(s) = s_lower.strip_prefix('.') {
            return Ok(FileTypeArg::Ext(s.to_string()));
        }

        // try parse as FileType
        if let Ok(ft) = FileType::from_str(&s_lower) {
            return Ok(FileTypeArg::Type(ft));
        }

        // try parse as FileCategory
        if let Ok(cat) = FileCategory::parse_with_aliases(&s_lower) {
            return Ok(FileTypeArg::FileCategory(cat));
        }

        // fallback to group
        Ok(FileTypeArg::Group(s.to_string()))
    }
}
