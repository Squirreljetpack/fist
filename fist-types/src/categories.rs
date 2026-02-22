use cli_boilerplate_automation::bath::{PathExt, split_ext};
use std::path::Path;

pub use super::categories_phf::*;

// drawn from crates eza and file-format
#[derive(Debug, Clone, strum_macros::EnumString, PartialEq, Eq, std::hash::Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[strum(serialize_all = "lowercase")]
pub enum FileCategory {
    /// Animated images, icons, cursors, raster graphics and vector graphics.
    Image,
    /// Moving images, possibly with color and coordinated sound.
    Video,
    /// Musics, sound effects, and spoken audio recordings.
    Audio,
    /// Lossless music.
    Lossless,
    /// Cryptocurrency files.
    Crypto,
    /// Word processing and desktop publishing documents.
    Document,
    /// Files and directories stored in a single, possibly compressed, archive.
    Compressed,
    /// Temporary files.
    Temp,
    /// Compilation artifacts.
    Compiled,
    /// A “build file is something that can be run or activated somehow in order to kick off the build of a project. It’s usually only present in directories full of source code.
    Build,
    /// Source code.
    Source,
    /// Configuration and structured data
    Configuration,
    /// Plain text.
    Text,

    /// Organized collections of data.
    Database,
    /// Visual information using graphics and spatial relationships.
    Diagram,
    /// Floppy disk images, optical disc images and virtual machine disks.
    Disk,
    /// Electronic books.
    Ebook,
    /// Machine-executable code, virtual machine code and shared libraries.
    Executable,
    /// Typefaces used for displaying text on screen or in print.
    Font,
    /// Mathematical formulas.
    Formula,
    /// Collections of geospatial features, GPS tracks and other location-related files.
    Geospatial,
    /// Data that provides information about other data.
    Metadata,
    /// 3D models, CAD drawings, and other types of files used for creating or displaying 3D images.
    Model,
    /// Collections of files bundled together for software distribution.
    Package,
    /// Lists of audio or video files, organized in a specific order for sequential playback.
    Playlist,
    /// Slide shows.
    Presentation,
    /// Copies of a read-only memory chip of computers, cartridges, or other electronic devices.
    Rom,
    /// Data in tabular form.
    Spreadsheet,
    /// Subtitles and captions.
    Subtitle,

    /// Email data.
    Email,
    /// Academic and publishing.
    Academic,
    /// Markdown.
    Markdown,

    /// Data which do not fit in any of the other kinds.
    Other,
}

impl FileCategory {
    pub fn exts(&self) -> Vec<&'static str> {
        EXTENSION_TYPES
            .entries()
            .filter_map(|(ext, cat)| (cat == self).then_some(*ext))
            .collect()
    }

    // todo: flesh out
    pub fn get(path: &Path) -> Option<FileCategory> {
        let name = path.filename();
        let ext = split_ext(&name)[1];

        // Case-insensitive readme check
        if name.to_lowercase().starts_with("readme") {
            return Some(Self::Build);
        }

        // Check full filename mapping
        if let Some(file_type) = FILENAME_TYPES.get(&*name) {
            return Some(file_type.clone());
        }

        // Check extension mapping
        if let Some(file_type) = EXTENSION_TYPES.get(ext) {
            return Some(file_type.clone());
        }

        // Temporary file check (~ or #…#)
        if name.ends_with('~') || (name.starts_with('#') && name.ends_with('#')) {
            return Some(Self::Temp);
        }

        // Modification of original: just do a ext check
        if EXTENSION_TYPES.get(ext) == Some(&Self::Compiled) {
            return Some(Self::Compiled);
        };

        None
    }

    // TODO: flesh out
    #[cfg(feature = "file-format")]
    pub fn from_fileformat(format: file_format::FileFormat) -> Self {
        use FileCategory::*;
        use file_format::Kind;

        match format.kind() {
            Kind::Archive | Kind::Compressed => Compressed,
            Kind::Audio => Audio,
            Kind::Database => Database,
            Kind::Diagram => Diagram,
            Kind::Disk => Disk,
            Kind::Document => Document,
            Kind::Ebook => Ebook,
            Kind::Executable => Executable,
            Kind::Font => Font,
            Kind::Formula => Formula,
            Kind::Geospatial => Geospatial,
            Kind::Image => Image,
            Kind::Metadata => Metadata,
            Kind::Model => Model,
            Kind::Other if format.media_type().starts_with("text/") => Text,
            Kind::Other => Other,
            Kind::Package => Package,
            Kind::Playlist => Playlist,
            Kind::Presentation => Presentation,
            Kind::Rom => Rom,
            Kind::Spreadsheet => Spreadsheet,
            Kind::Subtitle => Subtitle,
            Kind::Video => Video,
            _ => Other,
        }
    }

    pub fn is_text(&self) -> bool {
        use FileCategory::*;
        matches!(self, Text | Source | Configuration)
    }

    /// Command-line parsing (FromStr is separate)
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

    pub fn from_mime(mime: &str) -> Self {
        // documents = [
        // "application/pdf",
        // "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        // "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        // "application/msword",
        // "application/vnd.ms-powerpoint",
        // "application/vnd.oasis.opendocument.text",
        // ]

        // spreadsheets = [
        // "application/vnd.ms-excel",
        // "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        // "application/vnd.ms-excel.sheet.macroenabled.12",
        // "application/vnd.ms-excel.sheet.binary.macroenabled.12",
        // "application/vnd.ms-excel.addin.macroenabled.12",
        // "application/vnd.ms-excel",
        // "application/vnd.oasis.opendocument.spreadsheet",
        // ]

        // text_and_markup = [
        // "text/plain",
        // "text/markdown",
        // "text/x-markdown",
        // "text/html",
        // "application/xhtml+xml",
        // "application/xml",
        // "text/xml",
        // "image/svg+xml",
        // "text/x-rst",
        // "text/x-org",
        // "application/rtf",
        // "text/rtf",
        // "text/x-djot",
        // ]

        // structured_data = [
        // "application/json",
        // "text/json",
        // "application/x-yaml",
        // "text/yaml",
        // "text/x-yaml",
        // "application/toml",
        // "text/toml",
        // "text/csv",
        // "text/tab-separated-values",
        // ]

        // email = [
        // "message/rfc822",
        // "application/vnd.ms-outlook",
        // ]

        // images = [
        // "image/png",
        // "image/jpeg",
        // "image/jpg",
        // "image/webp",
        // "image/bmp",
        // "image/x-bmp",
        // "image/x-ms-bmp",
        // "image/tiff",
        // "image/x-tiff",
        // "image/gif",
        // "image/jp2",
        // "image/jpx",
        // "image/jpm",
        // "image/mj2",
        // "image/x-jbig2",
        // "image/x-portable-anymap",
        // ]

        // archives = [
        // "application/zip",
        // "application/x-zip-compressed",
        // "application/x-tar",
        // "application/x-bzip2",
        // "application/x-xz",
        // "application/tar",
        // "application/x-gtar",
        // "application/x-ustar",
        // "application/x-7z-compressed",
        // "application/gzip",
        // "application/x-gzip",
        // ]

        // academic_and_publishing = [
        // "application/x-latex",
        // "text/x-tex",
        // "application/epub+zip",
        // "application/x-bibtex",
        // "application/x-biblatex",
        // "application/x-typst",
        // "application/x-ipynb+json",
        // "application/x-fictionbook+xml",
        // "application/docbook+xml",
        // "application/x-jats+xml",
        // "application/x-opml+xml",
        // "application/x-research-info-systems",
        // "application/x-endnote+xml",
        // "application/x-pubmed",
        // "application/csl+json",
        // ]

        // markdown_variants = [
        // "text/x-commonmark",
        // "text/x-gfm",
        // "text/x-multimarkdown",
        // "text/x-markdown-extra",
        // "text/x-djot",
        // ]

        // other_formats = [
        // "text/x-mdoc",
        // "text/troff",
        // "text/x-pod",
        // "text/x-dokuwiki",
        // ]
        todo!()
    }
}
