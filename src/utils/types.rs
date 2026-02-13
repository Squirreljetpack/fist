// split out from categories.rs and filetypes.rs so we can include it with build.rs

// drawn from crates eza and file-format
#[derive(Debug, Clone, PartialEq, Eq, std::hash::Hash, strum_macros::EnumString)]
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

// ------------------------------------------------------------------------------------------

#[derive(
    Debug,
    strum_macros::Display,
    strum_macros::EnumString,
    Clone,
    Copy,
    PartialEq,
    Eq,
    std::hash::Hash,
)]
#[strum(serialize_all = "kebab-case")] // optional: converts variants to kebab-case by default
pub enum FileType {
    #[strum(serialize = "f")]
    File,
    #[strum(serialize = "d")]
    Directory,
    #[strum(serialize = "l")]
    Symlink,
    #[strum(serialize = "b")]
    BlockDevice,
    #[strum(serialize = "c")]
    CharDevice,
    #[strum(serialize = "x")]
    Executable,
    #[strum(serialize = "e")]
    Empty,
    #[strum(serialize = "s")]
    Socket,
    #[strum(serialize = "p")]
    Pipe,
}
