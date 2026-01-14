use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use arrayvec::ArrayVec;
use cli_boilerplate_automation::vec_;
use serde::{Deserialize, Deserializer};

use crate::abspath::AbsPath;
use crate::arr;
use crate::cli::BINARY_SHORT;
use crate::cli::paths::{current_exe, metadata_viewer_path, pager_path, show_error_path};
use crate::lessfilter::file_rule::FileData;
use crate::lessfilter::helpers::{header_viewer, image_viewer, infer_editor, infer_visual};
use crate::lessfilter::{LessfilterConfig, Preset};

#[derive(Debug, Hash, PartialEq, Eq, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum Action {
    // core
    Directory,
    Text,
    Image,
    Metadata,

    Open, // always system open
    Header,
    None,
    // todo: Url,
    /// Key to a custom [action](super::config::CustomActions)
    Custom(String),
}

impl<'de> Deserialize<'de> for Action {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        match s.to_lowercase().as_str() {
            "directory" => Ok(Action::Directory),
            "text" => Ok(Action::Text),
            "image" => Ok(Action::Image),
            "open" => Ok(Action::Open),
            "metadata" => Ok(Action::Metadata),
            "header" => Ok(Action::Header),
            "none" => Ok(Action::None),
            _ => Ok(Action::Custom(s)),
        }
    }
}

#[allow(warnings)]
impl Action {
    /// submit to [crate::spawn::spawn]
    // pushing the path at the end may be a bit redundant but it shouldn't really matter either way
    // todo: some way of communicating which permissions are needed on the target
    pub fn to_progs(
        &self,
        path: &Path,
        preset: Preset,
    ) -> (ArrayVec<Vec<OsString>, 5>, [bool; 3]) {
        match self {
            _ if matches!(preset, Preset::Default) => Default::default(), // do nothing
            Action::Directory | Action::Text | Action::Image | Action::Metadata
                if matches!(preset, Preset::Open | Preset::Alternate) =>
            {
                (
                    arr![vec_![current_exe(), ":open", "--", path]],
                    [true, false, false],
                )
            }
            Action::Directory => match preset {
                Preset::Preview => (
                    arr![vec_![current_exe(), ":tool", "liza", ":u2", path]],
                    [true, false, true], // read + execute
                ),
                Preset::Display => (
                    arr![vec_![current_exe(), ":tool", "liza", ":u", path]],
                    [true, false, true],
                ),
                Preset::Extended => (
                    arr![vec_![current_exe(), ":tool", "liza", ":sa", path]],
                    [true, false, true],
                ),
                Preset::Info => (
                    arr![vec_![current_exe(), ":tool", "liza", ":x", path]],
                    [true, false, true],
                ),
                Preset::Edit => (arr![infer_visual(path)], [true, false, true]),
                Preset::Default | Preset::Open | Preset::Alternate => unreachable!(),
            },
            Action::Text => match preset {
                Preset::Edit => (arr![infer_editor(path)], [true, true, false]),
                Preset::Info => (
                    arr![vec_![metadata_viewer_path(), path]],
                    [true, false, false],
                ),
                Preset::Extended => (arr![vec_![pager_path(), path]], [true, false, false]),
                _ => (arr![vec_![pager_path(), path]], [true, false, false]),
            },
            Action::Image => match preset {
                Preset::Extended => (
                    arr![
                        header_viewer(path),
                        image_viewer(path),
                        vec_![metadata_viewer_path(), path]
                    ],
                    [true, false, false],
                ),
                Preset::Info => (
                    arr![header_viewer(path), vec_![metadata_viewer_path(), path]],
                    [true, false, false],
                ),
                Preset::Edit => (
                    arr![vec_![current_exe(), ":open", "--", path]],
                    [true, false, false],
                ),
                _ => (arr![image_viewer(path)], [true, false, false]),
            },
            Action::Open => (
                arr![vec_![current_exe(), ":open", "--", path]],
                [false, false, false],
            ),
            Action::Metadata => match preset {
                Preset::Extended => (
                    arr![vec_![metadata_viewer_path(), path]],
                    [true, false, false],
                ),
                Preset::Edit => (
                    arr![vec_![show_error_path(), "No handler configured."]],
                    [true, false, false],
                ),
                _ => (
                    arr![vec_![metadata_viewer_path(), path]],
                    [true, false, false],
                ),
            },
            Action::Header => (
                arr![vec_!["echo", "\\e[3;2m", path, "\\e[0m\n"]],
                [true, false, false],
            ),
            Action::Custom(_) => (arr![vec_![]], [false, false, false]),
            Action::None => (arr![], [false, false, false]),
        }
    }

    /// submit to [matchmaker::preview::previewer::Previewer]
    pub fn to_script(
        &self,
        target: &Path,
        preset: Preset,
    ) -> String {
        if let Some(p) = target.to_str() {
            match self {
                Action::Custom(s) => s.replace("'$target'", &format!("'{}'", p)),
                _ => todo!(),
            }
        } else {
            String::new()
        }
    }
}
