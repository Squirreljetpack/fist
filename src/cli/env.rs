use cli_boilerplate_automation::{bait::ResultExt, bog::BogOkExt, ebog};

use crate::utils::text::split_shell_like;

#[derive(Debug, Default)]
pub struct EnvOpts {
    pub ancestor: Option<usize>,
    pub opener: Option<String>,
}

impl EnvOpts {
    pub fn init() -> Option<Self> {
        let raw = std::env::var("FS_OPTS")._dbog()?;
        log::debug!("FS_OPTS: {raw}");

        let tokens = split_shell_like(&raw);

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
                _ => {
                    ebog!("Unknown FS_OPTS key: {k}");
                }
            }
        }

        log::debug!("FS_OPTS: {ret:?}");

        Some(ret)
    }
}
