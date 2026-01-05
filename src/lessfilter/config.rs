use std::collections::HashMap;

use crate::cli::{BINARY_SHORT, paths::current_exe};

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
    Preview,
    #[default]
    #[clap(alias = "d")]
    Display,
    #[clap(alias = "x")]
    Extended,
    #[clap(alias = "i")]
    Info,
    #[clap(alias = "o")]
    Open,
    #[clap(alias = "e")]
    Edit,
    #[clap(skip)]
    Default,
}

impl Preset {
    pub fn to_command_string(self) -> String {
        format!(
            "'{}' :tool lessfilter {self} {{}}",
            current_exe().to_str().unwrap_or(BINARY_SHORT),
        )
    }
}

#[derive(Default, Debug, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomActions(HashMap<String, String>);

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
