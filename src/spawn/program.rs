use std::ffi::OsString;
use std::path::PathBuf;

use cli_boilerplate_automation::ebog;
use log::warn;

use crate::abspath::AbsPath;
use anyhow::{Context, Result, anyhow};

/// A command which can be launched, corresponds to an [`crate::db::Entry`] of [`crate::db::DbTable::apps`].
/// The fields correspond to path, cmd of db::Entry
/// Since path is a primary key, a program stored in db can only have one collection of args
/// The existence of the program is not ever checked with which::which
pub enum Program {
    File(AbsPath, Option<String>), // app path, cmd override
    Cmd(PathBuf, Vec<OsString>),   // binary name in path, args string
}

impl Program {
    // bogs errors
    pub fn from_os_string(s: impl Into<OsString>) -> Option<Self> {
        let s = s.into();
        if let Some(s) = s.to_str() {
            let Ok(parts) = shell_words::split(s) else {
                ebog!("Failed to parse program: {s}");
                return None;
            };
            let mut parts: Vec<_> = parts.into_iter().map(OsString::from).collect();
            if parts.is_empty() {
                ebog!("Failed to parse program: {s}");
                return None;
            }
            let path = parts.remove(0).into();
            // let Ok(path) = which::which(parts.remove(0)) else {
            //     ebog!("Command not found: {s}");
            //     return None;
            // };

            Some(Self::Cmd(path, parts))
        } else {
            warn!(
                "Invalid string encountered when parsing into Program: {}",
                s.to_string_lossy()
            );
            Some(Self::Cmd(s.into(), vec![]))
        }
    }

    pub fn new_cmd(
        prog: &str,
        args: impl IntoIterator<Item = OsString>,
    ) -> Self {
        Self::Cmd(PathBuf::from(prog), args.into_iter().collect())
    }

    pub fn path(&self) -> AbsPath {
        match self {
            Self::File(p, _) => p.clone(),
            Self::Cmd(p, _) => AbsPath::new_unchecked(p),
        }
    }

    /// see [`crate::find::apps::collect_apps`]
    pub fn from_scanned_path(
        path: AbsPath,
        cmd: Option<String>,
    ) -> Self {
        Self::File(path, cmd)
    }

    /// parses of the exec string stored in db for ::File variant, for use in [`crate::spawn::spawn`]
    pub fn to_cmd(&self) -> Result<Vec<OsString>> {
        match self {
            Self::Cmd(path, args) => {
                let mut v = vec![path.into()];
                v.extend(args.iter().cloned());
                Ok(v)
            }
            Self::File(path, cmd_opt) => {
                let path_str = path
                    .to_str()
                    .ok_or_else(|| anyhow!("Invalid UTF-8 in file path: {:?}", path))?;

                if cfg!(target_os = "macos") {
                    if let Some(cmd_override) = cmd_opt {
                        let parts = shell_words::split(cmd_override).with_context(|| {
                            format!(
                                "Failed to parse cmd override for {:?}: {}",
                                path, cmd_override
                            )
                        })?;
                        Ok(parts.into_iter().map(OsString::from).collect())
                    } else if path_str.ends_with(".app") {
                        let mut v = vec![OsString::from("open"), OsString::from("-a")];
                        v.push(path.to_os_string());
                        Ok(v)
                    } else {
                        Err(anyhow!("Unsupported File type on macOS: {:?}", path))
                    }
                } else if cfg!(target_os = "linux") {
                    if path_str.ends_with(".desktop") {
                        // Extract part before % on Exec=, which stands in for a file identifier
                        // todo: lowpri: full support
                        let cmd_line = cmd_opt.as_deref().ok_or_else(|| {
                            anyhow!("Linux .desktop file {:?} requires a command override", path)
                        })?;
                        let exec_line = cmd_line.split('%').next().unwrap_or("");
                        let parts = shell_words::split(exec_line).with_context(|| {
                            format!("Failed to parse exec line for {:?}: {}", path, exec_line)
                        })?;

                        Ok(parts.into_iter().map(OsString::from).collect())
                    } else if let Some(cmd_override) = cmd_opt {
                        let parts = shell_words::split(cmd_override).with_context(|| {
                            format!(
                                "Failed to parse cmd override for {:?}: {}",
                                path, cmd_override
                            )
                        })?;
                        Ok(parts.into_iter().map(OsString::from).collect())
                    } else {
                        Err(anyhow!("Unsupported File type on Linux: {:?}", path))
                    }
                } else if cfg!(target_os = "windows") {
                    Err(anyhow!("File variant not implemented for Windows"))
                } else {
                    Err(anyhow!(
                        "Cannot construct command for File on unsupported platform: {:?}",
                        path
                    ))
                }
            }
        }
    }
}
