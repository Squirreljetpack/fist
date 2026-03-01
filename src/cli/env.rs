use std::sync::LazyLock;

use cli_boilerplate_automation::{
    bait::ResultExt,
    bog::BogOkExt,
    bring::{consume_escaped, parse_next_escape, split::split_whitespace_preserving_nesting},
    ebog, wbog,
};

#[cfg(feature = "mm_overrides")]
use {
    crate::cli::mm_partial_parse::{get_pairs, try_split_kv},
    anyhow::bail,
    matchmaker::config::PartialRenderConfig,
    matchmaker_partial::Set,
};

#[derive(Debug, Default)]
pub struct EnvOpts {
    pub ancestor: Option<usize>,
    pub opener: Option<String>,
    /// - "display":
    /// - "display-batch": This script is run on `panes.settings.display_script_batch_size` items at once. Each item provides 2 arguments: its full path, and the tail
    ///
    /// Strongly recommended to supply "display-batch" for performance.
    pub display: Option<Result<String, String>>,
    pub delim: Option<char>,
    pub output_separator: Option<String>,
    pub output_template: Option<String>,
}

impl EnvOpts {
    pub fn init() -> Option<Self> {
        let prefix = "Failed to parse FS_OPTS";
        let mut ret = Self::default();

        if let Ok(raw) = std::env::var("FS_OPTS")
            && !raw.is_empty()
        {
            let tokens = split_whitespace_preserving_nesting(&raw, None, Some(['[', ']']))
                .prefix(prefix)
                ._ebog()?;

            for tok in tokens {
                if tok.is_empty() {
                    continue;
                }
                let Some((k, v)) = tok.split_once('=') else {
                    ebog!("{prefix}"; "Invalid token (expected key=value)");
                    continue;
                };

                match k {
                    "ancestor" => {
                        ret.ancestor = v.parse().prefix("ancestor must be a number")._ebog_(prefix);
                    }
                    "opener" => {
                        ret.opener = Some(v.to_string());
                    }
                    "display" => {
                        ret.display = Some(Ok(v.to_string()));
                    }
                    "display-batch" => {
                        ret.display = Some(Err(v.to_string()));
                    }
                    "delim" => {
                        log::debug!("{v}, {}", v.len());
                        let mut chars = v.chars();
                        match chars.next() {
                            Some('\\') => match parse_next_escape(&mut chars) {
                                Ok(c) => ret.delim = Some(c),
                                Err(orig) => ret.delim = Some(orig),
                            },
                            Some(c) => {
                                if chars.next().is_some() {
                                    wbog!("Multi-character delimiter was ignored");
                                } else {
                                    ret.delim = Some(c)
                                }
                            }
                            None => {}
                        }
                    }
                    _ => {
                        ebog!("{prefix}"; "Unknown key: {k}");
                    }
                }
            }
        }

        if let Ok(raw) = std::env::var("FS_OUTPUT")
            && !raw.is_empty()
        {
            ret.output_template = Some(replace_escapes(&raw));
        }

        if let Ok(raw) = std::env::var("FS_OUTPUT_SEP")
            && !raw.is_empty()
        {
            ret.output_separator = Some(replace_escapes(&raw));
        }

        log::debug!("FS_OPTS: {ret:?}");

        Some(ret)
    }

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
        Self::parse_mm_overrides(args)._wbog()
    }

    #[cfg(feature = "mm_overrides")]
    fn parse_mm_overrides(args: Vec<String>) -> anyhow::Result<PartialRenderConfig> {
        let split = get_pairs(args)?;
        log::trace!("{split:?}");
        let mut partial = PartialRenderConfig::default();
        for (path, val) in split {
            let parts =
                match split_whitespace_preserving_nesting(&val, Some(['(', ')']), Some(['[', ']']))
                {
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
}

static ENV_OPTS: LazyLock<Option<EnvOpts>> = LazyLock::new(EnvOpts::init);

impl EnvOpts {
    pub fn with_env<F, R>(f: F) -> Option<R>
    where
        F: FnOnce(&EnvOpts) -> Option<R>,
    {
        ENV_OPTS.as_ref().and_then(f)
    }
}

fn replace_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            consume_escaped(&mut chars, &mut out);
            continue;
        }
        out.push(c);
    }
    out
}
