use crate::utils::{FileCategory, FileType};

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

impl FileCategory {
    /// Command-line parsing (FromStr is seperate)
    pub fn parse_with_aliases(s: &str) -> Result<FileCategory, String> {
        use FileCategory::*;

        // first try EnumString
        if let Ok(category) = s.parse::<FileCategory>() {
            return Ok(category);
        }

        // fallback to common aliases
        let s_lower = s.to_lowercase();
        let category = match s_lower.as_str() {
            "v" | "vid" => Video,
            "i" | "img" => Image,
            "a" | "aud" => Audio,
            "l" | "lossless" => Lossless,
            "z" | "zip" => Compressed,
            "t" | "tmp" => Temp,
            "o" | "obj" => Compiled,
            "b" => Build,
            "s" | "src" | "code" => Source,
            "conf" | "cfg" => Configuration,
            "txt" => Text,

            // new variants
            "db" => Database,
            "diag" => Diagram,
            "x" | "exe" => Executable,
            "geo" => Geospatial,
            "pkg" => Package,
            "ppt" => Presentation,
            "xl" | "xlsx" => Spreadsheet,
            "md" => Markdown,

            _ => return Err(s.to_string()),
        };

        Ok(category)
    }
}
