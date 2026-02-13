use std::{ffi::OsString, path::PathBuf};

use cli_boilerplate_automation::{vec_, wbog};

use super::FileTypeArg;
use crate::{
    cli::paths::{self, __cwd, __home},
    config::FdConfig,
    filters::Visibility,
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
    mut vis: Visibility,
    types: &[FileTypeArg],
    paths: &[OsString],
    fd_args: &[OsString],
    cfg: &FdConfig,
) -> Vec<OsString> {
    let mut ret = vec![];
    let mut no_ext = false;

    // Initialize extra args
    // Add base args and user/default args
    let mut extra_args: Vec<OsString> = cfg.base_args.iter().map(|s| s.into()).collect();

    if fd_args.is_empty() {
        extra_args.extend(cfg.default_args.iter().map(|s| s.into()));
    } else {
        extra_args.extend(fd_args.iter().cloned());
    }

    let full_path_pattern = extra_args.iter().any(|x| x == "--full-path" || x == "p");
    let glob_pattern = extra_args.iter().any(|x| x == "--glob" || x == "p");

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
                ret.push("-e".into());
                ret.push(inner.into());
            }
            FileTypeArg::Group(_grp) => {
                panic!("Custom groups not yet implemented")
            }
            FileTypeArg::NoExt => {
                no_ext = true;
            }
        }
    }

    if no_ext {
        if full_path_pattern || glob_pattern {
            wbog!("no_ext filtering is not supported with flags: --glob, --full-path, or -p");
        } else {
            ret.push("--and".into());
            ret.push("^[^.]+$".into()); // no extension
        }
    }

    // "smart" vis settings
    // allow base_args to override the default true for vis.follow
    vis.no_follow |= cfg.base_args.iter().any(|x| x == "--no-follow");
    if !vis.no_follow {
        ret.push("--follow".into());
    }
    // auto-enable hidden on some patterns
    if !full_path_pattern && !vis.hidden_files && !vis.all() {
        if (glob_pattern
            && paths
                .last()
                .is_some_and(|s| s.to_string_lossy().starts_with('.')))
            || (!glob_pattern
                && paths
                    .last()
                    .is_some_and(|s| s.to_string_lossy().starts_with("^\\.")))
        {
            vis.hidden = true;
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
            if paths::__cwd() == paths::__home() {
                if let Some(excls) = cfg.exclusions.get(&PathBuf::from("~")) {
                    exclusions.extend(excls.iter().cloned());
                }
            } else if let Ok(stripped) = __cwd().strip_prefix(__home())
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

    ret.append(&mut extra_args);

    ret
}
