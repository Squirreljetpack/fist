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

use cli_boilerplate_automation::bath::{basename, split_ext};
use phf::{Map, phf_map};

// use crate::fs::File;

#[derive(Debug, Clone, PartialEq, Eq, std::hash::Hash)]
pub enum FileCategory {
    Image,
    Video,
    Music,
    Lossless, // Lossless music, rather than any other kind of data...
    Crypto,
    Document,
    Compressed,
    Temp,
    Compiled,
    Build, // A “build file is something that can be run or activated somehow in order to
    // kick off the build of a project. It’s usually only present in directories full of
    // source code.
    Source,
    Configuration, // add configuration
    Text,
}

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
    "aac"        => FileCategory::Music, // Advanced Audio Coding
    "m4a"        => FileCategory::Music,
    "mka"        => FileCategory::Music,
    "mp2"        => FileCategory::Music,
    "mp3"        => FileCategory::Music,
    "ogg"        => FileCategory::Music,
    "opus"       => FileCategory::Music,
    "wma"        => FileCategory::Music,
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
    "a"          => FileCategory::Compiled, // Unix static library
    "bundle"     => FileCategory::Compiled, // macOS application bundle
    "class"      => FileCategory::Compiled, // Java class file
    "cma"        => FileCategory::Compiled, // OCaml bytecode library
    "cmi"        => FileCategory::Compiled, // OCaml interface
    "cmo"        => FileCategory::Compiled, // OCaml bytecode object
    "cmx"        => FileCategory::Compiled, // OCaml bytecode object for inlining
    "dll"        => FileCategory::Compiled, // Windows dynamic link library
    "dylib"      => FileCategory::Compiled, // Mach-O dynamic library
    "elc"        => FileCategory::Compiled, // Emacs compiled lisp
    "elf"        => FileCategory::Compiled, // Executable and Linkable Format
    "ko"         => FileCategory::Compiled, // Linux kernel module
    "lib"        => FileCategory::Compiled, // Windows static library
    "o"          => FileCategory::Compiled, // Compiled object file
    "obj"        => FileCategory::Compiled, // Compiled object file
    "pyc"        => FileCategory::Compiled, // Python compiled code
    "pyd"        => FileCategory::Compiled, // Python dynamic module
    "pyo"        => FileCategory::Compiled, // Python optimized code
    "so"         => FileCategory::Compiled, // Unix shared library
    "zwc"        => FileCategory::Compiled, // zsh compiled file
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
    "rst" => FileCategory::Text,
    "csv" => FileCategory::Text,
    "tsv" => FileCategory::Text,
    "log" => FileCategory::Text,
};

const IMAGE_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "arw", "avif", "bmp", "cbr", "cbz", "cr2", "dvi", "eps", "fodg", "gif", "heic", "heif", "ico", "j2c", "j2k", "jfi", "jfif", "jif", "jp2", "jpe", "jpeg", "jpf", "jpg", "jpx", "jxl", "kra", "krz", "nef", "odg", "orf", "pbm", "pgm", "png", "pnm", "ppm", "ps", "psd", "pxm", "raw", "qoi", "svg", "tif", "tiff", "webp", "xcf", "xpm",
};
const VIDEO_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "avi", "flv", "h264", "heics", "m2ts", "m2v", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "ogm", "ogv", "video", "vob", "webm", "wmv",
};
const MUSIC_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "aac", "m4a", "mka", "mp2", "mp3", "ogg", "opus", "wma",
};
const LOSSLESS_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "aif", "aifc", "aiff", "alac", "ape", "flac", "pcm", "wav", "wv",
};

const DOCUMENT_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "djvu", "doc", "docx", "eml", "fodp", "fods", "fodt", "fotd", "gdoc", "key", "keynote", "numbers", "odp", "ods", "odt", "pages", "pdf", "ppt", "pptx", "rtf", "xls", "xlsm", "xlsx",
};
const COMPRESSED_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "7z", "ar", "arj", "br", "bz", "bz2", "bz3", "cpio", "deb", "dmg", "gz", "iso", "lz", "lz4", "lzh", "lzma", "lzo", "phar", "qcow", "qcow2", "rar", "rpm", "tar", "taz", "tbz", "tbz2", "tc", "tgz", "tlz", "txz", "tz", "xz", "vdi", "vhd", "vhdx", "vmdk", "z", "zip", "zst",
};
const TEMP_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "bak", "bk", "bkp", "crdownload", "download", "fcbak", "fcstd1", "fdmdownload", "part", "swn", "swo", "swp", "tmp",
};

const SOURCE_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "applescript", "as", "asa", "awk", "c", "c++", "c++m", "cabal", "cc", "ccm", "clj", "cp", "cpp", "cppm", "cr", "cs", "css", "csx", "cu", "cxx", "cxxm", "cypher", "d", "dart", "di", "dpr", "el", "elm", "erl", "ex", "exs", "f", "f90", "fcmacro", "fcscript", "fnl", "for", "fs", "fsh", "fsi", "fsx", "gd", "go", "gradle", "groovy", "gvy", "h", "h++", "hh", "hpp", "hc", "hs", "htc", "hxx", "inc", "inl", "ino", "ipynb", "ixx", "java", "jl", "js", "jsx", "kt", "kts", "kusto", "less", "lhs", "lisp", "ltx", "lua", "m", "malloy", "matlab", "ml", "mli", "mn", "nb", "p", "pas", "php", "pl", "pm", "pod", "pp", "prql", "ps1", "psd1", "psm1", "purs", "py", "r", "rb", "rs", "rq", "sass", "scala", "scm", "scad", "scss", "sld", "sql", "ss", "swift", "tcl", "tex", "ts", "v", "vb", "vsh", "zig",
};
const CONFIGURATION_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "toml", "yaml", "yml", "json", "jsonc", "ini", "cfg", "conf", "properties", "env", "editorconfig", "gitignore", "gitattributes", "dockerfile", "dockerignore", "service", "socket",
};

static COMPILED_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "aux", "bbl", "bcf", "blg", "fdb_latexmk", "fls",
    "headfootlength", "lof", "lot", "out",
    "toc", "xdv", "a", "bundle", "class", "cma", "cmi", "cmo", "cmx", "dll", "dylib", "elc", "elf", "ko", "lib", "o", "obj", "pyc", "pyd", "pyo", "so", "zwc",
};

// todo: support filenames
const BUILD_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "ninja",
};
const CRYPTO_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "age", "asc", "cer", "crt", "csr", "gpg", "kbx", "md5", "p12", "pem", "pfx", "pgp", "pub", "sha1", "sha224", "sha256", "sha384", "sha512", "sig", "signature",
};

// readable, some of these are also in doc
const TEXT_EXTS: phf::Set<&'static str> = phf::phf_set! {
    "txt", "md", "rtf", "tex", "rst", "csv", "tsv", "log"
};

impl FileCategory {
    pub fn exts(&self) -> Vec<&'static str> {
        match self {
            Self::Image => IMAGE_EXTS.iter().cloned().collect(),
            Self::Video => VIDEO_EXTS.iter().cloned().collect(),
            Self::Music => MUSIC_EXTS.iter().cloned().collect(),
            Self::Lossless => LOSSLESS_EXTS.iter().cloned().collect(),
            Self::Crypto => CRYPTO_EXTS.iter().cloned().collect(),
            Self::Document => DOCUMENT_EXTS.iter().cloned().collect(),
            Self::Compressed => COMPRESSED_EXTS.iter().cloned().collect(),
            Self::Temp => TEMP_EXTS.iter().cloned().collect(),
            Self::Compiled => COMPILED_EXTS
                .iter()
                .chain(COMPILED_EXTS.iter())
                .cloned()
                .collect(),
            Self::Build => BUILD_EXTS.iter().cloned().collect(),
            Self::Source => SOURCE_EXTS.iter().cloned().collect(),
            Self::Configuration => CONFIGURATION_EXTS.iter().cloned().collect(),
            Self::Text => TEXT_EXTS.iter().cloned().collect(),
        }
    }

    pub fn get(path: &Path) -> Option<FileCategory> {
        let name = basename(path);
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
        if COMPILED_EXTS.contains(ext) {
            return Some(Self::Compiled);
        };

        None
    }
}

use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
#[error("Invalid type: {0}")]
pub struct ParseFileTypeError(pub String);

impl FromStr for FileCategory {
    type Err = ParseFileTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "v" | "vid" | "video" => Ok(FileCategory::Video),
            "i" | "img" | "image" => Ok(FileCategory::Image),
            "a" | "aud" | "audio" => Ok(FileCategory::Music),
            "l" | "lossless" => Ok(FileCategory::Lossless),
            "crypto" => Ok(FileCategory::Crypto),
            "doc" | "document" => Ok(FileCategory::Document),
            "z" | "compressed" => Ok(FileCategory::Compressed),
            "t" | "tmp" | "temp" => Ok(FileCategory::Temp),
            // "b" | "build" => Ok(FileCategory::Build),
            "s" | "src" | "source" | "code" => Ok(FileCategory::Source),
            "o" | "compiled" => Ok(FileCategory::Compiled),
            "conf" => Ok(FileCategory::Configuration),
            "txt" => Ok(FileCategory::Text),
            _ => Err(ParseFileTypeError(s.to_string())),
        }
    }
}
