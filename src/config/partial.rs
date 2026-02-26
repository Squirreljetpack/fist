use matchmaker::config::PartialRenderConfig;

use crate::run::FsPane;

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MatchmakerOverrides {
    pub fullscreen: bool,
    pub reverse: bool, // unimplemented

    pub app: PartialRenderConfig,
    pub history: PartialRenderConfig,
    pub nav: PartialRenderConfig,
    pub stream: PartialRenderConfig,
    pub find: PartialRenderConfig,
    pub search: PartialRenderConfig,
    pub custom: PartialRenderConfig,
    pub settings: PartialRenderConfig,
}

impl MatchmakerOverrides {
    pub fn get(
        &self,
        pane: &FsPane,
    ) -> &PartialRenderConfig {
        match pane {
            FsPane::Custom { .. } => &self.custom,
            FsPane::Stream { .. } => &self.stream,
            FsPane::Find { .. } => &self.find,
            FsPane::Files { .. } | FsPane::Folders { .. } => &self.history,
            FsPane::Apps { .. } => &self.app,
            FsPane::Nav { .. } => &self.nav,
            FsPane::Search { .. } => &self.search,
        }
    }
}
