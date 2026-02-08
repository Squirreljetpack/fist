// SPDX-FileCopyrightText: 2024 Christina Sørensen
// SPDX-License-Identifier: EUPL-1.2
//
// SPDX-FileCopyrightText: 2023-2024 Christina Sørensen, eza contributors
// SPDX-FileCopyrightText: 2014 Benjamin Sago
// SPDX-License-Identifier: MIT
//! Tests for various types of file (video, image, compressed, etc).
//!
//! Currently this is dependent on the file’s name and extension, because
//! those are the only metadata that we have access to without reading the
//! file’s contents.
//!
//! # Contributors
//! Please keep these lists sorted. If you're using vim, :sort i

use std::path::Path;

use cli_boilerplate_automation::bath::{filename, split_ext};
use phf::{Map, phf_map};

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

use thiserror::Error;

#[derive(Debug, Error)]
#[error("Invalid type: {0}")]
pub struct ParseFileTypeError(pub String);

/// Mapping from full filenames to file type.
const FILENAME_TYPES: Map<&'static str, FileCategory> = phf_map! {
    /* Immediate file - kick off the build of a project */
    "Brewfile"           => FileCategory::Build,
    "bsconfig.json"      => FileCategory::Build,
    "BUILD"              => FileCategory::Build,
    "BUILD.bazel"        => FileCategory::Build,
    "build.gradle"       => FileCategory::Build,
    "build.sbt"          => FileCategory::Build,
    "build.xml"          => FileCategory::Build,
    "Cargo.toml"         => FileCategory::Build,
    "CMakeLists.txt"     => FileCategory::Build,
    "composer.json"      => FileCategory::Build,
    "configure"          => FileCategory::Build,
    "Containerfile"      => FileCategory::Build,
    "Dockerfile"         => FileCategory::Build,
    "Earthfile"          => FileCategory::Build,
    "flake.nix"          => FileCategory::Build,
    "Gemfile"            => FileCategory::Build,
    "GNUmakefile"        => FileCategory::Build,
    "Gruntfile.coffee"   => FileCategory::Build,
    "Gruntfile.js"       => FileCategory::Build,
    "jsconfig.json"      => FileCategory::Build,
    "Justfile"           => FileCategory::Build,
    "justfile"           => FileCategory::Build,
    "Makefile"           => FileCategory::Build,
    "makefile"           => FileCategory::Build,
    "meson.build"        => FileCategory::Build,
    "mix.exs"            => FileCategory::Build,
    "package.json"       => FileCategory::Build,
    "Pipfile"            => FileCategory::Build,
    "PKGBUILD"           => FileCategory::Build,
    "Podfile"            => FileCategory::Build,
    "pom.xml"            => FileCategory::Build,
    "Procfile"           => FileCategory::Build,
    "pyproject.toml"     => FileCategory::Build,
    "Rakefile"           => FileCategory::Build,
    "RoboFile.php"       => FileCategory::Build,
    "SConstruct"         => FileCategory::Build,
    "tsconfig.json"      => FileCategory::Build,
    "Vagrantfile"        => FileCategory::Build,
    "webpack.config.cjs" => FileCategory::Build,
    "webpack.config.js"  => FileCategory::Build,
    "WORKSPACE"          => FileCategory::Build,
    /* Cryptology files */
    "id_dsa"             => FileCategory::Crypto,
    "id_ecdsa"           => FileCategory::Crypto,
    "id_ecdsa_sk"        => FileCategory::Crypto,
    "id_ed25519"         => FileCategory::Crypto,
    "id_ed25519_sk"      => FileCategory::Crypto,
    "id_rsa"             => FileCategory::Crypto,
};

/// Mapping from lowercase file extension to file type.  If an image, video, music, or lossless
/// extension is added also update the extension icon map.
const EXTENSION_TYPES: Map<&'static str, FileCategory> = phf_map! {
    /* Immediate file - kick off the build of a project */
    "ninja"      => FileCategory::Build,
    /* Image files */
    "arw"        => FileCategory::Image,
    "avif"       => FileCategory::Image,
    "bmp"        => FileCategory::Image,
    "cbr"        => FileCategory::Image,
    "cbz"        => FileCategory::Image,
    "cr2"        => FileCategory::Image,
    "dvi"        => FileCategory::Image,
    "eps"        => FileCategory::Image,
    "fodg"       => FileCategory::Image,
    "gif"        => FileCategory::Image,
    "heic"       => FileCategory::Image,
    "heif"       => FileCategory::Image,
    "ico"        => FileCategory::Image,
    "j2c"        => FileCategory::Image,
    "j2k"        => FileCategory::Image,
    "jfi"        => FileCategory::Image,
    "jfif"       => FileCategory::Image,
    "jif"        => FileCategory::Image,
    "jp2"        => FileCategory::Image,
    "jpe"        => FileCategory::Image,
    "jpeg"       => FileCategory::Image,
    "jpf"        => FileCategory::Image,
    "jpg"        => FileCategory::Image,
    "jpx"        => FileCategory::Image,
    "jxl"        => FileCategory::Image,
    "kra"        => FileCategory::Image,
    "krz"        => FileCategory::Image,
    "nef"        => FileCategory::Image,
    "odg"        => FileCategory::Image,
    "orf"        => FileCategory::Image,
    "pbm"        => FileCategory::Image,
    "pgm"        => FileCategory::Image,
    "png"        => FileCategory::Image,
    "pnm"        => FileCategory::Image,
    "ppm"        => FileCategory::Image,
    "ps"         => FileCategory::Image,
    "psd"        => FileCategory::Image,
    "pxm"        => FileCategory::Image,
    "raw"        => FileCategory::Image,
    "qoi"        => FileCategory::Image,
    "svg"        => FileCategory::Image,
    "tif"        => FileCategory::Image,
    "tiff"       => FileCategory::Image,
    "webp"       => FileCategory::Image,
    "xcf"        => FileCategory::Image,
    "xpm"        => FileCategory::Image,
    /* Video files */
    "avi"        => FileCategory::Video,
    "flv"        => FileCategory::Video,
    "h264"       => FileCategory::Video,
    "heics"      => FileCategory::Video,
    "m2ts"       => FileCategory::Video,
    "m2v"        => FileCategory::Video,
    "m4v"        => FileCategory::Video,
    "mkv"        => FileCategory::Video,
    "mov"        => FileCategory::Video,
    "mp4"        => FileCategory::Video,
    "mpeg"       => FileCategory::Video,
    "mpg"        => FileCategory::Video,
    "ogm"        => FileCategory::Video,
    "ogv"        => FileCategory::Video,
    "video"      => FileCategory::Video,
    "vob"        => FileCategory::Video,
    "webm"       => FileCategory::Video,
    "wmv"        => FileCategory::Video,
    /* Music files */
    "aac"        => FileCategory::Audio, // Advanced Audio Coding
    "m4a"        => FileCategory::Audio,
    "mka"        => FileCategory::Audio,
    "mp2"        => FileCategory::Audio,
    "mp3"        => FileCategory::Audio,
    "ogg"        => FileCategory::Audio,
    "opus"       => FileCategory::Audio,
    "wma"        => FileCategory::Audio,
    /* Lossless music, rather than any other kind of data... */
    "aif"        => FileCategory::Lossless,
    "aifc"       => FileCategory::Lossless,
    "aiff"       => FileCategory::Lossless,
    "alac"       => FileCategory::Lossless,
    "ape"        => FileCategory::Lossless,
    "flac"       => FileCategory::Lossless,
    "pcm"        => FileCategory::Lossless,
    "wav"        => FileCategory::Lossless,
    "wv"         => FileCategory::Lossless,
    /* Cryptology files */
    "age"        => FileCategory::Crypto, // age encrypted file
    "asc"        => FileCategory::Crypto, // GnuPG ASCII armored file
    "cer"        => FileCategory::Crypto,
    "crt"        => FileCategory::Crypto,
    "csr"        => FileCategory::Crypto, // PKCS#10 Certificate Signing Request
    "gpg"        => FileCategory::Crypto, // GnuPG encrypted file
    "kbx"        => FileCategory::Crypto, // GnuPG keybox
    "md5"        => FileCategory::Crypto, // MD5 checksum
    "p12"        => FileCategory::Crypto, // PKCS#12 certificate (Netscape)
    "pem"        => FileCategory::Crypto, // Privacy-Enhanced Mail certificate
    "pfx"        => FileCategory::Crypto, // PKCS#12 certificate (Microsoft)
    "pgp"        => FileCategory::Crypto, // PGP security key
    "pub"        => FileCategory::Crypto, // Public key
    "sha1"       => FileCategory::Crypto, // SHA-1 hash
    "sha224"     => FileCategory::Crypto, // SHA-224 hash
    "sha256"     => FileCategory::Crypto, // SHA-256 hash
    "sha384"     => FileCategory::Crypto, // SHA-384 hash
    "sha512"     => FileCategory::Crypto, // SHA-512 hash
    "sig"        => FileCategory::Crypto, // GnuPG signed file
    "signature"  => FileCategory::Crypto, // e-Filing Digital Signature File (India)
    /* Document files */
    "djvu"       => FileCategory::Document,
    "doc"        => FileCategory::Document,
    "docx"       => FileCategory::Document,
    "eml"        => FileCategory::Document,
    "fodp"       => FileCategory::Document,
    "fods"       => FileCategory::Document,
    "fodt"       => FileCategory::Document,
    "fotd"       => FileCategory::Document,
    "gdoc"       => FileCategory::Document,
    "key"        => FileCategory::Document,
    "keynote"    => FileCategory::Document,
    "numbers"    => FileCategory::Document,
    "odp"        => FileCategory::Document,
    "ods"        => FileCategory::Document,
    "odt"        => FileCategory::Document,
    "pages"      => FileCategory::Document,
    "pdf"        => FileCategory::Document,
    "ppt"        => FileCategory::Document,
    "pptx"       => FileCategory::Document,
    "rtf"        => FileCategory::Document, // Rich Text Format
    "xls"        => FileCategory::Document,
    "xlsm"       => FileCategory::Document,
    "xlsx"       => FileCategory::Document,
    /* Compressed/archive files */
    "7z"         => FileCategory::Compressed, // 7-Zip
    "ar"         => FileCategory::Compressed,
    "arj"        => FileCategory::Compressed,
    "br"         => FileCategory::Compressed, // Brotli
    "bz"         => FileCategory::Compressed, // bzip
    "bz2"        => FileCategory::Compressed, // bzip2
    "bz3"        => FileCategory::Compressed, // bzip3
    "cpio"       => FileCategory::Compressed,
    "deb"        => FileCategory::Compressed, // Debian
    "dmg"        => FileCategory::Compressed,
    "gz"         => FileCategory::Compressed, // gzip
    "iso"        => FileCategory::Compressed,
    "lz"         => FileCategory::Compressed,
    "lz4"        => FileCategory::Compressed,
    "lzh"        => FileCategory::Compressed,
    "lzma"       => FileCategory::Compressed,
    "lzo"        => FileCategory::Compressed,
    "phar"       => FileCategory::Compressed, // PHP PHAR
    "qcow"       => FileCategory::Compressed,
    "qcow2"      => FileCategory::Compressed,
    "rar"        => FileCategory::Compressed,
    "rpm"        => FileCategory::Compressed,
    "tar"        => FileCategory::Compressed,
    "taz"        => FileCategory::Compressed,
    "tbz"        => FileCategory::Compressed,
    "tbz2"       => FileCategory::Compressed,
    "tc"         => FileCategory::Compressed,
    "tgz"        => FileCategory::Compressed,
    "tlz"        => FileCategory::Compressed,
    "txz"        => FileCategory::Compressed,
    "tz"         => FileCategory::Compressed,
    "xz"         => FileCategory::Compressed,
    "vdi"        => FileCategory::Compressed,
    "vhd"        => FileCategory::Compressed,
    "vhdx"       => FileCategory::Compressed,
    "vmdk"       => FileCategory::Compressed,
    "z"          => FileCategory::Compressed,
    "zip"        => FileCategory::Compressed,
    "zst"        => FileCategory::Compressed, // Zstandard
    /* Temporary files */
    "bak"        => FileCategory::Temp,
    "bk"         => FileCategory::Temp,
    "bkp"        => FileCategory::Temp,
    "crdownload" => FileCategory::Temp,
    "download"   => FileCategory::Temp,
    "fcbak"      => FileCategory::Temp,
    "fcstd1"     => FileCategory::Temp,
    "fdmdownload"=> FileCategory::Temp,
    "part"       => FileCategory::Temp,
    "swn"        => FileCategory::Temp,
    "swo"        => FileCategory::Temp,
    "swp"        => FileCategory::Temp,
    "tmp"        => FileCategory::Temp,
    /* Compiler output files */
    "a"              => FileCategory::Compiled, // Unix static library
    "aux"            => FileCategory::Compiled, // LaTeX auxiliary file
    "bbl"            => FileCategory::Compiled, // BibTeX bibliography output
    "bcf"            => FileCategory::Compiled, // BibLaTeX control file
    "blg"            => FileCategory::Compiled, // BibTeX log file
    "bundle"         => FileCategory::Compiled, // macOS application bundle
    "class"          => FileCategory::Compiled, // Java class file
    "cma"            => FileCategory::Compiled, // OCaml bytecode library
    "cmi"            => FileCategory::Compiled, // OCaml interface
    "cmo"            => FileCategory::Compiled, // OCaml bytecode object
    "cmx"            => FileCategory::Compiled, // OCaml bytecode object for inlining
    "dll"            => FileCategory::Compiled, // Windows dynamic link library
    "dylib"          => FileCategory::Compiled, // Mach-O dynamic library
    "elc"            => FileCategory::Compiled, // Emacs compiled lisp
    "elf"            => FileCategory::Compiled, // Executable and Linkable Format
    "fdb_latexmk"    => FileCategory::Compiled, // latexmk database
    "fls"            => FileCategory::Compiled, // LaTeX file list
    "headfootlength" => FileCategory::Compiled, // TeX derived layout data
    "ko"             => FileCategory::Compiled, // Linux kernel module
    "lib"            => FileCategory::Compiled, // Windows static library
    "lof"            => FileCategory::Compiled, // LaTeX list of figures
    "lot"            => FileCategory::Compiled, // LaTeX list of tables
    "o"              => FileCategory::Compiled, // Compiled object file
    "obj"            => FileCategory::Compiled, // Compiled object file
    "out"            => FileCategory::Compiled, // LaTeX/TeX auxiliary output
    "pyc"            => FileCategory::Compiled, // Python compiled code
    "pyd"            => FileCategory::Compiled, // Python dynamic module
    "pyo"            => FileCategory::Compiled, // Python optimized code
    "so"             => FileCategory::Compiled, // Unix shared library
    "toc"            => FileCategory::Compiled, // LaTeX table of contents
    "xdv"            => FileCategory::Compiled, // XeTeX extended DVI
    "zwc"            => FileCategory::Compiled, // zsh compiled file


    /* Source code files */
    "applescript"=> FileCategory::Source, // Apple script
    "as"         => FileCategory::Source, // Action script
    "asa"        => FileCategory::Source, // asp
    "awk"        => FileCategory::Source, // awk
    "c"          => FileCategory::Source, // C/C++
    "c++"        => FileCategory::Source, // C/C++
    "c++m"       => FileCategory::Source, // C/C++ module
    "cabal"      => FileCategory::Source, // Cabal
    "cc"         => FileCategory::Source, // C/C++
    "ccm"        => FileCategory::Source, // C/C++ module
    "clj"        => FileCategory::Source, // Clojure
    "cp"         => FileCategory::Source, // C/C++ Xcode
    "cpp"        => FileCategory::Source, // C/C++
    "cppm"       => FileCategory::Source, // C/C++ module
    "cr"         => FileCategory::Source, // Crystal
    "cs"         => FileCategory::Source, // C#
    "css"        => FileCategory::Source, // css
    "csx"        => FileCategory::Source, // C#
    "cu"         => FileCategory::Source, // CUDA
    "cxx"        => FileCategory::Source, // C/C++
    "cxxm"       => FileCategory::Source, // C/C++ module
    "cypher"     => FileCategory::Source, // Cypher (query language)
    "d"          => FileCategory::Source, // D
    "dart"       => FileCategory::Source, // Dart
    "di"         => FileCategory::Source, // D
    "dpr"        => FileCategory::Source, // Delphi Pascal
    "el"         => FileCategory::Source, // Lisp
    "elm"        => FileCategory::Source, // Elm
    "erl"        => FileCategory::Source, // Erlang
    "ex"         => FileCategory::Source, // Elixir
    "exs"        => FileCategory::Source, // Elixir
    "f"          => FileCategory::Source, // Fortran
    "f90"        => FileCategory::Source, // Fortran
    "fcmacro"    => FileCategory::Source, // FreeCAD macro
    "fcscript"   => FileCategory::Source, // FreeCAD script
    "fnl"        => FileCategory::Source, // Fennel
    "for"        => FileCategory::Source, // Fortran
    "fs"         => FileCategory::Source, // F#
    "fsh"        => FileCategory::Source, // Fragment shader
    "fsi"        => FileCategory::Source, // F#
    "fsx"        => FileCategory::Source, // F#
    "gd"         => FileCategory::Source, // GDScript
    "go"         => FileCategory::Source, // Go
    "gradle"     => FileCategory::Source, // Gradle
    "groovy"     => FileCategory::Source, // Groovy
    "gvy"        => FileCategory::Source, // Groovy
    "h"          => FileCategory::Source, // C/C++ header
    "h++"        => FileCategory::Source, // C/C++ header
    "hh"         => FileCategory::Source, // C/C++ header
    "hpp"        => FileCategory::Source, // C/C++ header
    "hc"         => FileCategory::Source, // HolyC
    "hs"         => FileCategory::Source, // Haskell
    "htc"        => FileCategory::Source, // JavaScript
    "hxx"        => FileCategory::Source, // C/C++ header
    "inc"        => FileCategory::Source,
    "inl"        => FileCategory::Source, // C/C++ Microsoft
    "ino"        => FileCategory::Source, // Arduino
    "ipynb"      => FileCategory::Source, // Jupyter Notebook
    "ixx"        => FileCategory::Source, // C/C++ module
    "java"       => FileCategory::Source, // Java
    "jl"         => FileCategory::Source, // Julia
    "js"         => FileCategory::Source, // JavaScript
    "jsx"        => FileCategory::Source, // React
    "kt"         => FileCategory::Source, // Kotlin
    "kts"        => FileCategory::Source, // Kotlin
    "kusto"      => FileCategory::Source, // Kusto (query language)
    "less"       => FileCategory::Source, // less
    "lhs"        => FileCategory::Source, // Haskell
    "lisp"       => FileCategory::Source, // Lisp
    "ltx"        => FileCategory::Source, // LaTeX
    "lua"        => FileCategory::Source, // Lua
    "m"          => FileCategory::Source, // Matlab
    "malloy"     => FileCategory::Source, // Malloy (query language)
    "matlab"     => FileCategory::Source, // Matlab
    "ml"         => FileCategory::Source, // OCaml
    "mli"        => FileCategory::Source, // OCaml
    "mn"         => FileCategory::Source, // Matlab
    "nb"         => FileCategory::Source, // Mathematica
    "p"          => FileCategory::Source, // Pascal
    "pas"        => FileCategory::Source, // Pascal
    "php"        => FileCategory::Source, // PHP
    "pl"         => FileCategory::Source, // Perl
    "pm"         => FileCategory::Source, // Perl
    "pod"        => FileCategory::Source, // Perl
    "pp"         => FileCategory::Source, // Puppet
    "prql"       => FileCategory::Source, // PRQL
    "ps1"        => FileCategory::Source, // PowerShell
    "psd1"       => FileCategory::Source, // PowerShell
    "psm1"       => FileCategory::Source, // PowerShell
    "purs"       => FileCategory::Source, // PureScript
    "py"         => FileCategory::Source, // Python
    "r"          => FileCategory::Source, // R
    "rb"         => FileCategory::Source, // Ruby
    "rs"         => FileCategory::Source, // Rust
    "rq"         => FileCategory::Source, // SPARQL (query language)
    "sass"       => FileCategory::Source, // Sass
    "scala"      => FileCategory::Source, // Scala
    "scm"        => FileCategory::Source, // Scheme
    "scad"       => FileCategory::Source, // OpenSCAD
    "scss"       => FileCategory::Source, // Sass
    "sld"        => FileCategory::Source, // Scheme Library Definition
    "sql"        => FileCategory::Source, // SQL
    "ss"         => FileCategory::Source, // Scheme Source
    "swift"      => FileCategory::Source, // Swift
    "tcl"        => FileCategory::Source, // TCL
    "tex"        => FileCategory::Source, // LaTeX
    "ts"         => FileCategory::Source, // TypeScript
    "v"          => FileCategory::Source, // V
    "vb"         => FileCategory::Source, // Visual Basic
    "vsh"        => FileCategory::Source, // Vertex shader
    "zig"        => FileCategory::Source, // Zig

    // Modification: Add Conf
    "toml"         => FileCategory::Configuration,
    "yaml"         => FileCategory::Configuration,
    "yml"          => FileCategory::Configuration,
    "json"         => FileCategory::Configuration,
    "jsonc"        => FileCategory::Configuration,
    "ini"          => FileCategory::Configuration,
    "cfg"          => FileCategory::Configuration,
    "conf"         => FileCategory::Configuration,
    "properties"   => FileCategory::Configuration,
    "env"          => FileCategory::Configuration,
    "editorconfig" => FileCategory::Configuration,
    "gitignore"    => FileCategory::Configuration,
    "gitattributes"=> FileCategory::Configuration,
    "dockerfile"   => FileCategory::Configuration,
    "dockerignore" => FileCategory::Configuration,
    "service"      => FileCategory::Configuration,
    "socket"       => FileCategory::Configuration,

    "txt" => FileCategory::Text,
    "md"  => FileCategory::Text,
    // rtf and tex are accounted for
    "rst" => FileCategory::Text,
    "csv" => FileCategory::Text,
    "tsv" => FileCategory::Text,
    "log" => FileCategory::Text,
};

impl FileCategory {
    pub fn exts(&self) -> Vec<&'static str> {
        EXTENSION_TYPES
            .entries()
            .filter_map(|(ext, cat)| (cat == self).then_some(*ext))
            .collect()
    }

    // todo: flesh out
    pub fn get(path: &Path) -> Option<FileCategory> {
        let name = filename(path);
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

    pub fn parse_with_aliases(s: &str) -> Result<FileCategory, ParseFileTypeError> {
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

            _ => return Err(ParseFileTypeError(s.to_string())),
        };

        Ok(category)
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
