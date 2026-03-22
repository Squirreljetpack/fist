use serde::{Deserialize, Serialize, Serializer};

use crate::run::stash::STASH_BUILTINS;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StashMode {
    pub scratch: bool,
    pub target: bool,
    pub batch: bool,
    pub parallel: u8,
    pub on_fail: BreakStrategy,
    pub strategy: ExecuteStrategy,
    pub unique: StashAddRule,
}

impl Default for StashMode {
    fn default() -> Self {
        Self {
            scratch: false,
            target: false,
            batch: false,
            parallel: 1,
            on_fail: BreakStrategy::None,
            strategy: ExecuteStrategy::None,
            unique: Default::default(),
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
    Interrupt(String), // todo: requires batch
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum StashAddRule {
    False,
    #[default]
    True,
    Limit(u8),
}

#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct StashLogicConfig {
    pub modes: Vec<(String, StashMode)>,
}

use serde::de::{self, Deserializer, MapAccess, Visitor};
use std::fmt;
impl<'de> Deserialize<'de> for StashLogicConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StashLogicVisitor;

        impl<'de> Visitor<'de> for StashLogicVisitor {
            type Value = StashLogicConfig;

            fn expecting(
                &self,
                f: &mut fmt::Formatter,
            ) -> fmt::Result {
                f.write_str("a map of stash mode names to configurations")
            }

            fn visit_map<M>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut modes = Vec::new();

                while let Some((name, mode)) = map.next_entry::<String, StashMode>()? {
                    if STASH_BUILTINS.contains(&name.as_str()) {
                        return Err(de::Error::custom(format!(
                            "Reserved name '{}' cannot be used for a custom stash mode",
                            name
                        )));
                    }
                    modes.push((name, mode));
                }

                Ok(StashLogicConfig { modes })
            }
        }

        deserializer.deserialize_map(StashLogicVisitor)
    }
}

impl<'de> Deserialize<'de> for StashAddRule {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = StashAddRule;

            fn expecting(
                &self,
                f: &mut std::fmt::Formatter,
            ) -> std::fmt::Result {
                f.write_str("a boolean or an integer (u8)")
            }

            fn visit_bool<E>(
                self,
                v: bool,
            ) -> Result<Self::Value, E> {
                Ok(if v {
                    StashAddRule::True
                } else {
                    StashAddRule::False
                })
            }

            fn visit_u64<E>(
                self,
                v: u64,
            ) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if v <= u8::MAX as u64 {
                    Ok(StashAddRule::Limit(v as u8))
                } else {
                    Err(E::custom("number too large for u8"))
                }
            }

            fn visit_i64<E>(
                self,
                v: i64,
            ) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if (0..=u8::MAX as i64).contains(&v) {
                    Ok(StashAddRule::Limit(v as u8))
                } else {
                    Err(E::custom("number out of range for u8"))
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}
impl StashAddRule {
    pub fn is_true(&self) -> bool {
        !matches!(self, StashAddRule::False)
    }
}
impl Serialize for StashAddRule {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            StashAddRule::False => serializer.serialize_bool(false),
            StashAddRule::True => serializer.serialize_bool(true),
            StashAddRule::Limit(n) => serializer.serialize_u8(n),
        }
    }
}
