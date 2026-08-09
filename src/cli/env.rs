//! MM_OPTS* environment overrides (`mm_overrides` feature).
//!
//! FS_OPTS/FS_OUTPUT were removed in the matchmaker migration — their knobs are
//! now CLI flags (`--delim`/`--record-sep` on `:custom`, `--format`/`--sep`/`--opener`
//! via `OutputOpts`).

#[cfg(feature = "mm_overrides")]
use {
    crate::cli::mm_partial_parse::{get_pairs, try_split_kv},
    anyhow::bail,
    cba::{
        bait::ResultExt,
        bog::BogOkExt,
        bring::split::split_whitespace_preserving_nesting,
        wbog,
    },
    matchmaker::config::PartialRenderConfig,
    matchmaker_partial::Set,
};

#[cfg(feature = "mm_overrides")]
/// Gets a PartialRenderConfig by reading from environment variables MM_OPTS0, MM_OPTS1...
/// Warns and stops reading on encountering improper top-level nesting.
/// Returns None upon encountering parse errors after (the top-level split).
pub fn get_mm_partial() -> Option<PartialRenderConfig> {
    let mut args = vec![];
    let i = 0;
    while let Ok(val) = std::env::var(format!("MM_OPTS{i}"))
        && !val.is_empty()
    {
        match split_whitespace_preserving_nesting(&val, Some(['(', ')']), Some(['[', ']'])) {
            Ok(parts) => {
                args.extend(parts);
            }
            Err(n) => {
                if n > 0 {
                    wbog!(
                        "Stopped reading for overrides at MM_OPTS{i}: Encountered {} unclosed parentheses",
                        n
                    )
                } else {
                    wbog!(
                        "Stopped reading for overrides at MM_OPTS{i}: Extra closing parenthesis at index {}",
                        -n
                    )
                }
                break;
            }
        };
    }
    if args.is_empty() {
        return None;
    }
    parse_mm_overrides(args)._wbog()
}

#[cfg(feature = "mm_overrides")]
fn parse_mm_overrides(args: Vec<String>) -> anyhow::Result<PartialRenderConfig> {
    let split = get_pairs(args)?;
    log::trace!("{split:?}");
    let mut partial = PartialRenderConfig::default();
    for (path, val) in split {
        let parts =
            match split_whitespace_preserving_nesting(&val, Some(['(', ')']), Some(['[', ']'])) {
                Ok(mut parts) => {
                    let is_binds =
                        parts.len() == 1 && ["binds", "b"].contains(&parts[0].as_ref());
                    try_split_kv(&mut parts, is_binds)?;
                    parts
                }
                Err(n) => {
                    if n > 0 {
                        bail!("Encountered {} unclosed parentheses", n)
                    } else {
                        bail!("Extra closing parenthesis at index {}", -n)
                    }
                }
            };

        log::trace!("{parts:?}");

        partial
            .set(path.as_slice(), &parts)
            .prefix(format!("Invalid value for {}", path.join(".")))?;
    }

    Ok(partial)
}
