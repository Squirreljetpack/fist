//! Archive format detection: extension map with a magic-byte fallback.
//!
//! Compressed streams (gz/bz2/xz/zst) all collapse into their single
//! [`Format`] variant regardless of whether they wrap a tar; the
//! tar-or-bare decision happens on the decoded bytes at list/extract time
//! (see [`codec::peek_tar`]).

use std::fs;
use std::io::Read;
use std::path::Path;

/// An archive format this engine can handle.
///
/// Variants only exist when their cargo feature is enabled, so `Format` is
/// never constructed for a format the build cannot extract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    #[cfg(feature = "zip")]
    Zip,
    #[cfg(feature = "tar")]
    Tar,
    #[cfg(feature = "ar")]
    Ar,
    #[cfg(feature = "rar")]
    Rar,
    #[cfg(feature = "sevenz")]
    SevenZ,
    #[cfg(feature = "gz")]
    Gz,
    #[cfg(feature = "bz2")]
    Bz2,
    #[cfg(feature = "xz")]
    Xz,
    #[cfg(feature = "zst")]
    Zst,
}

impl Format {
    /// Human-readable name for logs and errors.
    pub fn id(self) -> &'static str {
        match self {
            #[cfg(feature = "zip")]
            Format::Zip => "zip",
            #[cfg(feature = "tar")]
            Format::Tar => "tar",
            #[cfg(feature = "ar")]
            Format::Ar => "ar",
            #[cfg(feature = "rar")]
            Format::Rar => "rar",
            #[cfg(feature = "sevenz")]
            Format::SevenZ => "7z",
            #[cfg(feature = "gz")]
            Format::Gz => "gz",
            #[cfg(feature = "bz2")]
            Format::Bz2 => "bz2",
            #[cfg(feature = "xz")]
            Format::Xz => "xz",
            #[cfg(feature = "zst")]
            Format::Zst => "zst",
        }
    }
}

/// Detects the archive format of `path`: by file name first, then by
/// sniffing content. `None` when no enabled format matches either.
pub fn detect(path: &Path) -> Option<Format> {
    detect_by_extension(path).or_else(|| detect_by_content(path))
}

fn detect_by_extension(path: &Path) -> Option<Format> {
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    Some(match name.rsplit('.').next()? {
        _ if has_suffix(&name, ".tar.gz")
            || has_suffix(&name, ".tgz")
            || has_suffix(&name, ".taz") =>
        {
            fmt_gz()?
        }
        _ if has_suffix(&name, ".tar.bz2")
            || has_suffix(&name, ".tbz")
            || has_suffix(&name, ".tbz2") =>
        {
            fmt_bz2()?
        }
        _ if has_suffix(&name, ".tar.xz") || has_suffix(&name, ".txz") => fmt_xz()?,
        _ if has_suffix(&name, ".tar.zst") || has_suffix(&name, ".tzst") => fmt_zst()?,
        "tar" => fmt_tar()?,
        "zip" | "jar" => fmt_zip()?,
        "ar" | "a" | "rlib" => fmt_ar()?,
        "rar" => fmt_rar()?,
        "7z" => fmt_7z()?,
        "gz" => fmt_gz()?,
        "bz2" => fmt_bz2()?,
        "xz" => fmt_xz()?,
        "zst" => fmt_zst()?,
        _ => return None,
    })
}

fn has_suffix(
    name: &str,
    suffix: &str,
) -> bool {
    name.len() > suffix.len() && name.ends_with(suffix)
}

// Each helper maps to its variant only when that format's feature is on;
// otherwise it yields `None` and detection falls through to the next rule.

fn fmt_zip() -> Option<Format> {
    #[cfg(feature = "zip")]
    {
        Some(Format::Zip)
    }
    #[cfg(not(feature = "zip"))]
    {
        None
    }
}

fn fmt_tar() -> Option<Format> {
    #[cfg(feature = "tar")]
    {
        Some(Format::Tar)
    }
    #[cfg(not(feature = "tar"))]
    {
        None
    }
}

fn fmt_ar() -> Option<Format> {
    #[cfg(feature = "ar")]
    {
        Some(Format::Ar)
    }
    #[cfg(not(feature = "ar"))]
    {
        None
    }
}

fn fmt_rar() -> Option<Format> {
    #[cfg(feature = "rar")]
    {
        Some(Format::Rar)
    }
    #[cfg(not(feature = "rar"))]
    {
        None
    }
}

fn fmt_7z() -> Option<Format> {
    #[cfg(feature = "sevenz")]
    {
        Some(Format::SevenZ)
    }
    #[cfg(not(feature = "sevenz"))]
    {
        None
    }
}

fn fmt_gz() -> Option<Format> {
    #[cfg(feature = "gz")]
    {
        Some(Format::Gz)
    }
    #[cfg(not(feature = "gz"))]
    {
        None
    }
}

fn fmt_bz2() -> Option<Format> {
    #[cfg(feature = "bz2")]
    {
        Some(Format::Bz2)
    }
    #[cfg(not(feature = "bz2"))]
    {
        None
    }
}

fn fmt_xz() -> Option<Format> {
    #[cfg(feature = "xz")]
    {
        Some(Format::Xz)
    }
    #[cfg(not(feature = "xz"))]
    {
        None
    }
}

fn fmt_zst() -> Option<Format> {
    #[cfg(feature = "zst")]
    {
        Some(Format::Zst)
    }
    #[cfg(not(feature = "zst"))]
    {
        None
    }
}

/// Magic-byte sniffing over the first block of the file. Used when the
/// file name gives nothing away (or names a format whose feature is off).
/// A full tar block is read so extensionless tars are recognized by their
/// `ustar` magic at offset 257.
fn detect_by_content(path: &Path) -> Option<Format> {
    let mut buf = [0u8; 512];
    let n = fs::File::open(path).ok()?.read(&mut buf).ok()?;
    let b = &buf[..n];
    if b.len() >= 262 && b[257..262] == *b"ustar" {
        return fmt_tar();
    }
    Some(match b {
        b if b.starts_with(b"PK\x03\x04")
            || b.starts_with(b"PK\x05\x06")
            || b.starts_with(b"PK\x07\x08") =>
        {
            fmt_zip()?
        }
        b if b.starts_with(b"\x1f\x8b") => fmt_gz()?,
        b if b.starts_with(b"BZh") => fmt_bz2()?,
        b if b.starts_with(b"\xfd7zXZ\x00") => fmt_xz()?,
        b if b.starts_with(b"\x28\xb5\x2f\xfd") => fmt_zst()?,
        b if b.starts_with(b"!<arch>\n") => fmt_ar()?,
        b if b.starts_with(b"Rar!\x1a\x07") => fmt_rar()?,
        b if b.starts_with(b"7z\xbc\xaf\x27\x1c") => fmt_7z()?,
        _ => return None,
    })
}
