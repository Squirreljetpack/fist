#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MatchmakerOverrides {
    pub fullscreen: bool,
    pub reverse: bool, // unimplemented
}
