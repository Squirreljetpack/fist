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

use super::categories::FileCategory;
use phf::{Map, phf_map};

/// Mapping from full filenames to file type.
pub const FILENAME_TYPES: Map<&'static str, FileCategory> = phf_map! {
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
pub const EXTENSION_TYPES: Map<&'static str, FileCategory> = phf_map! {
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
