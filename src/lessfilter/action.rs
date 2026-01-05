use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use arrayvec::ArrayVec;
use cli_boilerplate_automation::vec_;
use serde::{Deserialize, Deserializer};

use crate::abspath::AbsPath;
use crate::arr;
use crate::cli::BINARY_SHORT;
use crate::cli::paths::{current_exe, metadata_viewer_path, pager_path};
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

    // header
    Header,
    None,
    // Url,
    Custom(String), // script
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
        target: &Path,
        preset: Preset,
    ) -> (ArrayVec<Vec<OsString>, 5>, [bool; 3]) {
        match self {
            _ if matches!(preset, Preset::Default) => Default::default(),
            Action::Directory => match preset {
                Preset::Preview => (
                    arr![vec_![current_exe(), ":tool", "lz", ":u2", target]],
                    [true, false, true], // read + execute
                ),
                Preset::Display => (
                    arr![vec_![current_exe(), ":tool", "lz", ":u", target]],
                    [true, false, true],
                ),
                Preset::Extended => (
                    arr![vec_![current_exe(), ":tool", "lz", ":sa", target]],
                    [true, false, true],
                ),
                Preset::Info => (
                    arr![vec_![current_exe(), ":tool", "lz", ":x", target]],
                    [true, false, true],
                ),
                Preset::Open => (
                    arr![vec_![current_exe(), ":open", target]],
                    [true, false, true],
                ),
                Preset::Edit => (
                    arr![infer_visual(target)],
                    [true, true, true], // editing usually requires write + execute if a script
                ),
                Preset::Default => unreachable!(),
            },
            Action::Text => match preset {
                Preset::Edit => (arr![infer_editor(target)], [true, true, false]),
                Preset::Info => (
                    arr![vec_![metadata_viewer_path(), target]],
                    [true, false, false],
                ),
                Preset::Open => (
                    arr![vec_![current_exe(), ":open", target]],
                    [true, false, true],
                ),
                _ => (arr![vec_![pager_path(), target]], [true, false, false]),
            },
            Action::Image => match preset {
                Preset::Extended => (
                    arr![
                        header_viewer(target),
                        image_viewer(target),
                        vec_![metadata_viewer_path(), target]
                    ],
                    [true, false, false],
                ),
                Preset::Info => (
                    arr![header_viewer(target), vec_![metadata_viewer_path(), target]],
                    [true, false, false],
                ),
                Preset::Open => (
                    arr![vec_![current_exe(), ":open", target]],
                    [true, false, true],
                ),
                _ => (arr![image_viewer(target)], [true, false, false]),
            },
            Action::Metadata => (
                arr![vec_![metadata_viewer_path(), target]],
                [true, false, false],
            ),
            Action::Header => (
                arr![vec_!["echo", "\\e[3;2m", target, "\\e[0m\n"]],
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

// pub fn exec(
//     preset: Preset,
//     paths: &[PathBuf],
//     cfg: LessfilterConfig,
// ) {
//     let rules = if let Some(rules) = cfg.rules.get(&preset) {
//         rules
//     } else {
//         // No rules for this preset, do nothing
//         return;
//     };

//     for path in paths {
//         let path = if let Ok(p) = path.canonicalize() {
//             p
//         } else {
//             eprintln!("Error: Could not find file {}", path.to_string_lossy());
//             continue;
//         };

//         let data = FileData::new(AbsPath::new(path.clone()), cfg.infer);

//         if let Some(actions) = rules.get_best_match(&path, data) {
//             for action in actions {
//                 let script = action.to_script(&path, preset);
//                 let mut cmd = Command::new("sh");
//                 cmd.arg("-c").arg(&script);

//                 match cmd.status() {
//                     Ok(status) => {
//                         if !status.success() {
//                             eprintln!("Error: script failed for {}", path.to_string_lossy());
//                         }
//                     }
//                     Err(e) => {
//                         eprintln!(
//                             "Error: failed to execute script for {}: {}",
//                             path.to_string_lossy(),
//                             e
//                         );
//                     }
//                 }
//             }
//         }
//     }
// }
