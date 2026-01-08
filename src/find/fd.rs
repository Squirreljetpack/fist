use std::{ffi::OsString, path::PathBuf};

use cli_boilerplate_automation::{vec_, wbog};

use crate::{
    cli::paths::{self, __cwd, home_dir},
    config::FdConfig,
    filters::{SortOrder, Visibility},
    utils::{categories::FileCategory, filetypes::FileType},
};

// probably we can avoid clones + turn into const but this is easier
pub fn default_exclusions() -> Vec<String> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            vec_![
            "*.app/Contents/",
            ".DS_Store",
            ".localized",
            ".cache",
            ]
        } else if #[cfg(target_os = "windows")] {
            vec_![
            "Thumbs.db",
            "desktop.ini",
            ".cache",
            ]
        } else if #[cfg(target_os = "linux")] {
            vec_![
            ".cache",
            ]
        } else {
            vec_![
            ".cache",
            ]
        }
    }
}

pub fn default_home_exclusions() -> Vec<String> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "macos")] {
            vec_![
            "/Library/",
            "/micromamba/",
            ]
        } else if #[cfg(target_os = "windows")] {
            vec_![

            ]
        } else if #[cfg(target_os = "linux")] {
            vec_![
            "/.Trash",
            "/.local/share/Trash",
            ]
        } else {
            vec_![
            ]
        }
    }
}
pub fn build_fd_args(
    sort: SortOrder,
    vis: Visibility,
    types: &[FileTypeArg],
    paths: &[OsString],
    fd_args: &[OsString],
    cfg: &FdConfig,
) -> Vec<OsString> {
    let mut ret = vec![];
    let no_ext = false;

    // add -t + -e
    for t in types {
        match t {
            FileTypeArg::Type(inner) => {
                ret.push("-t".into());
                ret.push(inner.to_string().into());
            }
            FileTypeArg::FileCategory(cat) => {
                for x in cat.exts() {
                    ret.push("-e".into());
                    ret.push(x.into());
                }
                // This wouldn't work because there is no --or filter
                // if matches!(cat, FileCategory::Text) {
                //     no_ext = true;
                // }
            }
            FileTypeArg::Ext(inner) => {
                ret.push(inner.into());
            }
            FileTypeArg::Group(_grp) => {
                panic!("Custom groups not yet implemented")
            }
        }
    }

    // add vis flags
    if vis.all() {
        ret.push("--hidden".into());
        ret.push("--no-ignore".into());
    } else {
        if vis.hidden_files || vis.hidden {
            ret.push("--hidden".into());
        }
        if !vis.ignore {
            ret.push("--no-ignore".into());
        }
        // todo: lowpri: maybe we want to pop off other -t args as well
        if vis.dirs {
            ret.push("-t".into());
            ret.push("d".into());
        } else if vis.files {
            ret.push("-t".into());
            ret.push("f".into());
        }
    }

    // add -E
    if !vis.all() {
        let exclusions = {
            let mut exclusions = cfg
                .exclusions
                .get(&PathBuf::new())
                .cloned()
                .unwrap_or_else(default_exclusions);

            // todo: check full/replaced for all paths
            if paths::__cwd() == paths::home_dir() {
                if let Some(excls) = cfg.exclusions.get(&PathBuf::from("~")) {
                    exclusions.extend(excls.iter().cloned());
                }
            } else if let Ok(stripped) = __cwd().strip_prefix(home_dir())
                && let Some(excls) = cfg.exclusions.get(&PathBuf::from("~").join(stripped))
            {
                exclusions.extend(excls.iter().cloned());
            } else if let Some(excls) = cfg.exclusions.get(__cwd()) {
                exclusions.extend(excls.iter().cloned());
            }

            exclusions
        };

        for e in exclusions {
            ret.push("-E".into());
            ret.push(e.into());
        }
    }

    // pattern is the last arg
    if let Some(pattern) = paths.last() {
        ret.push("--and".into());
        ret.push(pattern.clone());
    }

    // add search paths
    for p in &paths[..paths.len().saturating_sub(1)] {
        ret.push("--search-path".into());
        ret.push(p.clone());
    }

    // Add base args and user/default args
    ret.extend(cfg.base_args.iter().map(|s| s.into()));
    if fd_args.is_empty() {
        ret.extend(cfg.default_args.iter().map(|s| s.into()));
    } else {
        ret.extend(fd_args.iter().cloned());
    }

    if no_ext {
        if ret
            .iter()
            .any(|x| x == "--glob" || x == "--full-path" || x == "p")
        {
            wbog!("No ext filtering is not supported with flags: --glob, --full-path, or -p");
        } else {
            ret.push("--and".into());
            ret.push("^[^.]+$".into()); // no extension
        }
    }

    ret
}

// ------- FileTypeArg -----------

/// Filetypes: [f, d, l, b, c, x, e, s p].
///
/// Categories: [img, vid, aud, doc, tmp, src, conf, …].
///
/// Ext: '.*'
///
/// Groups: Configurable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileTypeArg {
    Type(FileType),
    FileCategory(FileCategory),
    Ext(String),
    Group(String), // todo: custom groups in config
}

impl std::str::FromStr for FileTypeArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lower = s.to_lowercase();

        // extension if starts with "."
        if s_lower.starts_with('.') {
            return Ok(FileTypeArg::Ext(s_lower));
        }

        // try parse as FileType
        if let Ok(ft) = FileType::from_str(&s_lower) {
            return Ok(FileTypeArg::Type(ft));
        }

        // try parse as FileCategory
        if let Ok(cat) = FileCategory::from_str(&s_lower) {
            return Ok(FileTypeArg::FileCategory(cat));
        }

        // fallback to group
        Ok(FileTypeArg::Group(s.to_string()))
    }
}
