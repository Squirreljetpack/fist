use std::collections::HashMap;

use cli_boilerplate_automation::bo::load_type;

use crate::{
    cli::paths::{BINARY_SHORT, current_exe},
    lessfilter::RulesConfig,
};

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
    pub fn to_command_string(self) -> String {
        format!(
            "'{}' :tool lessfilter {self} {{}}",
            current_exe().to_str().unwrap_or(BINARY_SHORT),
        )
    }

    pub fn to_command_string_with_header(self) -> String {
        format!(
            "'{}' :tool lessfilter --header=true {self} {{}}",
            current_exe().to_str().unwrap_or(BINARY_SHORT),
        )
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LessfilterConfig {
    #[serde(flatten, default)]
    pub test: TestSettings,
    #[serde(default)]
    pub rules: RulesConfig,
    #[serde(default)]
    pub actions: CustomActions,
}

impl Default for LessfilterConfig {
    fn default() -> Self {
        let ret = toml::from_str(include_str!("../../assets/config/lessfilter.toml"));
        ret.unwrap()
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TestSettings {
    pub infer: bool,
}

impl Default for TestSettings {
    fn default() -> Self {
        Self { infer: true }
    }
}

/// Name => Shell Script
///
/// # Notes
/// Name is case insensitive
#[derive(Default, Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomActions(HashMap<String, String>);

// --------------------- BOILERPLATE ----------------------------------------

impl CustomActions {
    pub fn new() -> Self {
        CustomActions(HashMap::new())
    }
}

impl std::ops::Deref for CustomActions {
    type Target = HashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for CustomActions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
