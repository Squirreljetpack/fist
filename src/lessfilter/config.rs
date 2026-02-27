use std::{collections::HashMap, str::FromStr};

use cli_boilerplate_automation::define_collection_wrapper;
use mime_guess::Mime;

use crate::{
    cli::paths::{BINARY_SHORT, current_exe},
    lessfilter::{RulesConfig, file_rule::ParseFileRuleError},
};
use fist_types::{FileCategory, When};

#[derive(
    Default,
    Debug,
    Hash,
    PartialEq,
    Eq,
    Clone,
    Copy,
    serde::Serialize,
    serde::Deserialize,
    clap::ValueEnum,
    strum::Display,
    strum::EnumString,
)]
#[strum(serialize_all = "lowercase")]
pub enum Preset {
    #[clap(alias = "p")]
    /// For the f:ist preview pane.
    ///
    /// see [`matchmaker::preview`]
    Preview,
    #[default]
    #[clap(alias = "d")]
    /// For terminal display.
    Display,
    #[clap(alias = "x")]
    /// For terminal interaction/verbose display.
    Extended,
    #[clap(alias = "i")]
    /// Metadata/raw info.
    Info,
    #[clap(alias = "o")]
    /// System open.
    ///
    /// (By deferring to fs :open)
    Open,
    /// Alternate (custom) open
    Alternate,
    #[clap(alias = "e")]
    // For [`crate::run::FsAction::Advance`]
    Edit,
    #[clap(skip)]
    /// Default preset for configuration only
    Default,
}

impl Preset {
    pub fn to_command_string(
        self,
        header: When,
    ) -> String {
        let header = match header {
            When::Always => "--header=true",
            When::Never => "--header=false",
            When::Auto => "",
        };
        format!(
            "'{}' :tool lessfilter {header} {self} {{}}",
            current_exe().to_str().unwrap_or(BINARY_SHORT),
        )
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessfilterConfig {
    #[serde(flatten, default)]
    pub settings: LessfilterSettings,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(default)]
    pub actions: CustomActions,
    #[serde(default)]
    pub categories: Categories,
}

#[derive(Debug, Default, Copy, Clone, serde::Deserialize)]
pub enum InferMode {
    Guess,
    Infer,
    #[default]
    FileFormat,
}

impl Default for LessfilterConfig {
    fn default() -> Self {
        let ret = toml::from_str(include_str!("../../assets/config/lessfilter.toml"));
        ret.unwrap()
    }
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LessfilterSettings {
    pub infer: InferMode,
    /// This has to do with how a single action can sometimes be multiple command-line programs. This stops execution when any fail -- do not set.
    #[serde(skip)]
    pub early_exit: bool,
}

define_collection_wrapper!(
    /// Name => Shell Script
    ///
    /// # Notes
    /// Name is case insensitive
    ///
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    CustomActions: HashMap<String, String>
);
define_collection_wrapper!(
    #[derive(Debug)]
    Categories: HashMap<String, Vec<MimeString>>
);

#[derive(Default, Debug, serde::Deserialize, Clone)]
#[serde(default, transparent)]
pub struct MimeString(String);

impl FromStr for MimeString {
    type Err = ParseFileRuleError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.matches('/').count() != 1 {
            Err(ParseFileRuleError::InvalidMime)
        } else {
            Ok(MimeString(s.to_string()))
        }
    }
}

impl MimeString {
    pub fn equal(
        &self,
        mime: &Mime,
    ) -> bool {
        self.0 == mime.to_string()
    }

    pub fn matches_type(
        &self,
        r#type: &str,
    ) -> bool {
        let (type_, _subtype) = self.0.split_once('/').unwrap();
        type_.is_empty() || type_ == "*" || r#type == type_
    }

    pub fn matches_subtype(
        &self,
        subtype: &str,
    ) -> bool {
        let (_type_, subtype_) = self.0.split_once('/').unwrap();
        subtype_.is_empty() || subtype_ == "*" || subtype == subtype_
    }

    pub fn matches_any(&self) -> bool {
        let (type_, subtype) = self.0.split_once('/').unwrap();
        type_ == "*" && (subtype == "*" || subtype.is_empty())
    }
}

// --------------------- BOILERPLATE ----------------------------------------

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};

impl<'de> Deserialize<'de> for Categories {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let map = HashMap::<String, Vec<MimeString>>::deserialize(deserializer)?;

        for key in map.keys() {
            if FileCategory::from_str(key).is_ok() {
                return Err(D::Error::custom(format!(
                    "key '{}' must not be a valid FileCategory",
                    key
                )));
            }
        }

        Ok(Categories(map))
    }
}
