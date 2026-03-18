use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StashMode {
    pub exclusive: bool,
    pub target: bool,
    pub batch: bool,
    pub parallel: u8,
    pub on_fail: BreakStrategy,
    pub strategy: ExecuteStrategy,
    pub unique: bool,
}

impl Default for StashMode {
    fn default() -> Self {
        Self {
            exclusive: false,
            target: false,
            batch: false,
            parallel: 1,
            on_fail: BreakStrategy::None,
            strategy: ExecuteStrategy::None,
            unique: true,
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BreakStrategy {
    None,
    Pause, // pause remaining tasks
    End,   // mark all remaining tasks as failed
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecuteStrategy {
    // builtin
    Copy,
    Cut,
    Symlink,
    None,
    // custom
    Command(String),
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct StashLogicConfig {
    pub modes: IndexMap<String, StashMode>,
}

impl<'de> Deserialize<'de> for StashLogicConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let modes = IndexMap::<String, StashMode>::deserialize(deserializer)?;
        for name in modes.keys() {
            match name.as_str() {
                "app" | "copy" | "paste" | "cut" | "revert" | "symlink" => {
                    return Err(serde::de::Error::custom(format!(
                        "Reserved name '{}' cannot be used for a custom stash mode",
                        name
                    )));
                }
                _ => {}
            }
        }
        Ok(StashLogicConfig { modes })
    }
}
