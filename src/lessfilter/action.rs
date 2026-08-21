use std::ffi::OsString;
use std::path::{Path, PathBuf};

use arrayvec::ArrayVec;
use cba::vec_;
use serde::{Deserialize, Deserializer};

use crate::arr;
use crate::cli::paths::{current_exe, show_error_path};
use crate::lessfilter::Preset;
use crate::lessfilter::helpers::{application_icon_path, image_viewer, infer_editor, infer_visual};

#[derive(Debug, Hash, PartialEq, Eq, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum Action {
    // core
    Directory,
    Text,
    Image,
    Application,
    Extract,
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
            "application" => Ok(Action::Application),
            "extract" => Ok(Action::Extract),
            "open" => Ok(Action::Open),
            "metadata" => Ok(Action::Metadata),
            "header" => Ok(Action::Header),
            "none" => Ok(Action::None),
            _ => Ok(Action::Custom(s)),
        }
    }
}

/// One renderable step for a file: either an in-process display (header,
/// metadata, pager) or a program to spawn with args.
///
/// A `Vec<OsString>` command line converts into a [`Prog`](CommandStrategy::Prog)
/// wrap (first element = program, rest = args); an empty line becomes
/// [`None`](CommandStrategy::None).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStrategy {
    /// Render [PathBuf] through the in-process pager (bat + minus).
    Pager(PathBuf),
    /// Show the app header in-process.
    Header,
    /// Show metadata for [PathBuf] in-process.
    Metadata(PathBuf),
    /// Spawn the given program with args.
    Prog(OsString, Vec<OsString>),
    /// No program to spawn (empty command line).
    None,
}

/// Wrap a complete command line (program + args) into a spawn strategy; an
/// empty line becomes [`None`](CommandStrategy::None).
impl From<Vec<OsString>> for CommandStrategy {
    fn from(line: Vec<OsString>) -> Self {
        let mut it = line.into_iter();
        match it.next() {
            Some(prog) => CommandStrategy::Prog(prog, it.collect()),
            None => CommandStrategy::None,
        }
    }
}

#[allow(warnings)]
impl Action {
    pub fn to_progs(
        &self,
        path: &Path,
        preset: Preset,
    ) -> (ArrayVec<CommandStrategy, 5>, [bool; 3]) {
        use CommandStrategy::{Header, Metadata, Pager, Prog};

        match preset {
            Preset::Default => return Default::default(), // do nothing, should be unreachable
            Preset::Open | Preset::Alternate => {
                // standard actions in these 2 presets are just open
                if matches!(
                    self,
                    Action::Directory
                        | Action::Text
                        | Action::Image
                        | Action::Metadata
                        | Action::Application
                ) {
                    return (
                        arr![Prog(current_exe().into(), vec_![: ":open", "--", path])],
                        [true, false, false],
                    );
                }
            }
            _ => {}
        };

        match self {
            Action::Directory => match preset {
                Preset::Preview => (
                    arr![Prog(
                        current_exe().into(),
                        vec_![: ":tool", "liza", ":u2", "--", path]
                    )],
                    [true, false, true], // read + execute
                ),
                Preset::Display => (
                    arr![Prog(
                        current_exe().into(),
                        vec_![: ":tool", "liza", ":u", "--", path]
                    )],
                    [true, false, true],
                ),
                Preset::Extended => (
                    arr![
                        Header,
                        Prog(
                            current_exe().into(),
                            vec_![: ":tool", "liza", "::nav", ":a", "--", path]
                        )
                    ],
                    [true, false, true],
                ),
                Preset::Info => (
                    arr![Prog(
                        current_exe().into(),
                        vec_![: ":tool", "liza", ":sba", "--", path]
                    )],
                    [true, false, true],
                ),
                Preset::Edit => (arr![infer_visual(path).into()], [true, false, true]),
                Preset::Default | Preset::Open | Preset::Alternate | Preset::Alternate2 => {
                    unreachable!()
                }
            },
            Action::Text => match preset {
                Preset::Preview | Preset::Display => {
                    (arr![Pager(path.into())], [true, false, false])
                }
                // bat has a "native" header but using our app header is more consistent
                Preset::Extended => (
                    arr![Header, Pager(path.into()), Metadata(path.into())],
                    [true, false, false],
                ),
                Preset::Info => (
                    arr![
                        Prog(
                            current_exe().into(),
                            vec_![: ":tool", "liza", ":l", "--", path]
                        ),
                        Metadata(path.into())
                    ],
                    [true, false, false],
                ),
                Preset::Edit => (arr![infer_editor(path).into()], [true, true, false]),

                Preset::Default | Preset::Open | Preset::Alternate | Preset::Alternate2 => {
                    unreachable!()
                }
            },
            Action::Image => match preset {
                Preset::Preview | Preset::Display => {
                    (arr![image_viewer(path, None).into()], [true, false, false])
                }
                Preset::Extended => (
                    arr![
                        Header,
                        image_viewer(path, None).into(),
                        Metadata(path.into())
                    ],
                    [true, false, false],
                ),
                Preset::Info => (arr![Header, Metadata(path.into())], [true, false, false]),
                Preset::Edit => (
                    arr![Prog(current_exe().into(), vec_![: ":open", "--", path])],
                    [true, false, false],
                ),
                Preset::Default | Preset::Open | Preset::Alternate | Preset::Alternate2 => {
                    unreachable!()
                }
            },

            Action::Application => {
                let icon_path = application_icon_path(path);
                let display_path = icon_path.as_deref();

                match preset {
                    Preset::Preview | Preset::Display => {
                        // fallback to directory display if no icon found
                        let ac = if let Some(icon) = display_path {
                            image_viewer(icon, Some(16))
                        } else {
                            application_fallback(path)
                        };
                        (arr![ac.into()], [false, false, false])
                    }
                    Preset::Extended => {
                        let ac = if let Some(icon) = display_path {
                            image_viewer(icon, Some(16))
                        } else {
                            application_fallback(path)
                        };

                        (
                            arr![Header, ac.into(), Metadata(path.into())],
                            [true, false, true],
                        )
                    }
                    Preset::Info => (arr![Header, Metadata(path.into())], [true, false, true]),
                    Preset::Edit => (
                        arr![Prog(current_exe().into(), vec_![: ":open", "--", path])],
                        [false, false, false],
                    ),
                    Preset::Default | Preset::Open | Preset::Alternate | Preset::Alternate2 => {
                        unreachable!()
                    }
                }
            }

            // this action is basically just metadata except for extended
            // where we show stats (+ metadata if file)
            Action::Metadata => match preset {
                Preset::Extended | Preset::Info => {
                    // show file stats in addition to metadata like Info
                    let main = Prog(
                        current_exe().into(),
                        vec_![: ":tool", "liza", ":l", "--", path],
                    );
                    (
                        if path.is_file() {
                            arr![main, Metadata(path.into())]
                        } else {
                            // not a file => skip the metadata
                            // altho metadata = stats if !file, that's just a coincidence and semantically it only makes sense to show stats
                            arr![main]
                        },
                        [true, false, true],
                    )
                }
                Preset::Edit => {
                    let error_cmd = vec_![: show_error_path(), "No handler configured."];
                    (arr![error_cmd.into()], [true, false, false])
                }
                _ => (arr![Metadata(path.into())], [true, false, true]),
            },

            Action::Open => (
                arr![Prog(current_exe().into(), vec_![: ":open", "--", path])],
                [false, false, false],
            ),
            Action::Header => (arr![Header], [true, false, false]),
            Action::Custom(_) => unreachable!(),
            Action::Extract => unreachable!(),
            Action::None => (arr![], [false, false, false]),
        }
    }

    /// submit to [matchmaker::preview::previewer::Previewer]
    pub fn to_script(&self, target: &Path, preset: Preset) -> String {
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

fn application_fallback(path: &Path) -> Vec<OsString> {
    vec_![: current_exe(), ":tool", "liza", ":u2", "--", path]
}
