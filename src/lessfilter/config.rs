use std::{collections::HashMap, str::FromStr};

use cba::{bird::transform::camelcase_normalized, define_collection_wrapper};
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
    strum_macros::Display,
    strum_macros::EnumString,
)]
#[strum(serialize_all = "lowercase")]
pub enum Preset {
    #[clap(alias = "p")]
    /// For the f:st preview pane.
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
    /// The other presets' standard actions defer here.
    Open,
    /// Alternate (custom) open
    Alternate,
    /// Alternate (custom) open
    Alternate2,
    #[clap(alias = "e")]
    // For [`crate::run::FsAction::Advance`]
    Edit,
    #[clap(skip)]
    #[strum(disabled)]
    /// Default preset for configuration only
    Default,
}

impl Preset {
    /// The shell command that runs this preset through the `lessfilter` tool.
    ///
    /// `record` is false when the caller records the visit itself, which tells
    /// lessfilter to leave the history database alone (`--record=false`).
    pub fn to_command_string(
        self,
        header: When,
        record: bool,
    ) -> String {
        let header = match header {
            When::Always => "--header=true",
            When::Never => "--header=false",
            When::Auto => "",
        };
        let record = if record { "" } else { "--record=false " };
        format!(
            "'{}' :tool lessfilter {header} {record}{self} {{}}",
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

impl Default for LessfilterConfig {
    fn default() -> Self {
        let ret = toml::from_str(include_str!("../../assets/config/lessfilter.toml"));
        ret.unwrap()
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LessfilterSettings {
    #[serde(deserialize_with = "camelcase_normalized")]
    pub infer: InferMode,
    /// This has to do with how a single action can sometimes be multiple command-line programs. This stops execution when any fail -- do not set.
    #[serde(skip)]
    pub early_exit: bool,
    /// Presets whose invocations record the visited file into the history
    /// database. Opens record through this visit, so `open` is listed by
    /// default; `preview` never records.
    pub tracked_presets: Vec<Preset>,
    pub run: RunSettings,
}

#[derive(Default, Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RunSettings {
    pub image_viewer: Vec<String>,
}

#[derive(Debug, Default, Copy, Clone, serde::Deserialize)]
pub enum InferMode {
    Guess,
    Infer,
    #[default]
    FileFormat,
}

impl Default for LessfilterSettings {
    fn default() -> Self {
        Self {
            infer: Default::default(),
            early_exit: false,
            tracked_presets: vec![
                Preset::Edit,
                Preset::Alternate,
                Preset::Extended,
                Preset::Open,
            ],
            run: Default::default(),
        }
    }
}

define_collection_wrapper!(
    /// Name => Shell Script
    ///
    /// # Notes
    /// Name is matched exactly (case-sensitive).
    ///
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(transparent)]
    CustomActions: HashMap<String, String>
);
define_collection_wrapper!(
    #[derive(Debug)]
    Categories: HashMap<String, Vec<MimeString>>
);

impl Default for CustomActions {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Categories {
    fn default() -> Self {
        Self::new()
    }
}

// --------------------- BOILERPLATE ---------------------

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

impl std::fmt::Display for MimeString {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str(&self.0)
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

// ---------------------

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

        Ok(Categories::new_from(map))
    }
}
