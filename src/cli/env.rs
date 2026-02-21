use cli_boilerplate_automation::{
    bait::ResultExt, bog::BogOkExt, ebog, text::parse_next_escape, wbog,
};

use crate::utils::text::split_whitespace_keep_single_quotes;

#[derive(Debug, Default)]
pub struct EnvOpts {
    pub ancestor: Option<usize>,
    pub opener: Option<String>,
    // - "display":
    // - "display-batch": This script is run on `panes.settings.display_script_batch_size` items at once. Each item provides 2 arguments: its full path, and the tail
    // Strongly recommended to supply "display-batch" for performance.
    pub display: Option<Result<String, String>>,
    pub delim: Option<char>,
}

impl EnvOpts {
    pub fn init() -> Option<Self> {
        let raw = std::env::var("FS_OPTS").ok()?;
        let tokens = split_whitespace_keep_single_quotes(&raw);

        let mut ret = Self::default();

        for tok in tokens {
            let Some((k, v)) = tok.split_once('=') else {
                ebog!("Invalid FS_OPTS token (expected key=value)");
                continue;
            };

            match k {
                "ancestor" => {
                    ret.ancestor = v
                        .parse()
                        .prefix("ancestor must be a number")
                        ._ebog_("Invalid ENV");
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
                    ebog!("Unknown FS_OPTS key: {k}");
                }
            }
        }

        log::debug!("FS_OPTS: {ret:?}");

        Some(ret)
    }
}
