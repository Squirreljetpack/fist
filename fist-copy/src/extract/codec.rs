#![cfg_attr(
    not(any(feature = "gz", feature = "bz2", feature = "xz", feature = "zst")),
    allow(dead_code, unused_variables, unreachable_patterns)
)]

//! Compressed stream decoders and the tar-or-bare decision.
//!
//! Each enabled compression format contributes a [`decode`] arm wrapping a
//! byte source in the matching decompressing [`Read`]. Compound detection
//! ([`peek_tar`]) then decides on the decoded bytes whether the stream is
//! a tar archive or a bare compressed file.

use std::fs;
use std::io::Read;

use super::detect::Format;

pub(crate) const TAR_BLOCK: usize = 512;

/// A decompressing reader over an underlying byte source.
pub(crate) type BoxDecoder = Box<dyn Read + Send>;

/// Wraps `source` in the decoder for `format`. The caller guarantees the
/// format matches (detection ran first).
pub(crate) fn decode(
    source: fs::File,
    format: Format,
) -> BoxDecoder {
    match format {
        #[cfg(feature = "gz")]
        Format::Gz => Box::new(flate2::read::GzDecoder::new(source)),
        #[cfg(feature = "bz2")]
        Format::Bz2 => Box::new(bzip2::read::BzDecoder::new(source)),
        #[cfg(feature = "xz")]
        Format::Xz => Box::new(xz2::read::XzDecoder::new(source)),
        #[cfg(feature = "zst")]
        Format::Zst => match zstd::stream::read::Decoder::new(source) {
            Ok(d) => Box::new(d),
            // zstd's decoder construction is fallible only on allocation;
            // surface it as an empty erroring reader
            Err(e) => Box::new(ErrorReader(Some(e))),
        },
        _ => unreachable!("decode called for a non-stream format"),
    }
}

/// A reader that yields the stored error once, then ends. Stands in for
/// decoder constructors that can fail before reading.
struct ErrorReader(Option<std::io::Error>);

impl Read for ErrorReader {
    fn read(
        &mut self,
        _: &mut [u8],
    ) -> std::io::Result<usize> {
        match self.0.take() {
            Some(e) => Err(e),
            None => Ok(0),
        }
    }
}

/// Reads the first tar block of `decoded` and reports whether it parses as
/// a tar header: either the POSIX/GNU magic at offset 257 or a valid
/// header checksum over an all-zero name field.
///
/// The peeked block is returned so iteration can resume from it without
/// rewinding.
pub(crate) fn peek_tar(decoded: &mut BoxDecoder) -> std::io::Result<(bool, [u8; TAR_BLOCK])> {
    let mut block = [0u8; TAR_BLOCK];
    let mut read = 0;
    while read < TAR_BLOCK {
        let n = decoded.read(&mut block[read..])?;
        if n == 0 {
            break;
        }
        read += n;
    }
    let looks_like_tar =
        block[257..262] == *b"ustar" || (!block.iter().all(|b| *b == 0) && checksum_ok(&block));
    Ok((looks_like_tar, block))
}

/// Verifies the checksum of a 512-byte tar header, accepting both the
/// unsigned and signed byte conventions.
fn checksum_ok(block: &[u8; TAR_BLOCK]) -> bool {
    // the checksum field is zero-terminated octal, padded with spaces
    let field = &block[148..156];
    let digits: Vec<u8> = field
        .iter()
        .copied()
        .filter(|b| b.is_ascii_digit())
        .collect();
    if digits.is_empty() || digits.len() > 7 {
        return false;
    }
    let mut stored = 0u32;
    for d in digits {
        stored = stored * 8 + u32::from(d - b'0');
    }
    let unsigned: u32 = block[..148].iter().map(|b| u32::from(*b)).sum::<u32>()
        + 32 * 8 // the checksum field itself reads as spaces
        + block[156..].iter().map(|b| u32::from(*b)).sum::<u32>();
    let signed: i32 = block[..148]
        .iter()
        .map(|b| i32::from(*b as i8))
        .sum::<i32>()
        + 32 * 8
        + block[156..]
            .iter()
            .map(|b| i32::from(*b as i8))
            .sum::<i32>();
    unsigned == stored || signed == stored as i32
}
