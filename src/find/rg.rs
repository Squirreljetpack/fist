use std::{ffi::OsString, path::PathBuf};

use cba::vec_;

use crate::{
    cli::paths::{__cwd, __home},
    config::RgConfig,
};
use fist_types::When;
use fist_types::filters::{SortOrder, Visibility};

// pub fn is_inverted(
//     patterns: &[String],
//     rg_args: &[OsString],
//     cfg: &RgConfig,
// ) -> bool {
//     if patterns.is_empty()
//         && cfg
//             .empty_pattern
//             .as_deref()
//             .is_some_and(|x| x.starts_with("-v "))
//     {
//         return true;
//     };
//     if rg_args.iter().take_while(|v| *v != "--").any(|v| v == "-v") {
//         return true;
//     }
//     let mut extra_args: Vec<OsString> = cfg.base_args.iter().map(|s| s.into()).collect();
//     if rg_args.is_empty() {
//         extra_args.extend(cfg.default_args.iter().map(|s| s.into()));
//     }
//     if rg_args.iter().any(|v| v == "-v") {
//         return true;
//     }
//     false
// }

pub fn is_inverted(args: &[OsString]) -> bool {
    args.iter().take_while(|v| *v != "--").any(|v| v == "-v")
}

#[allow(clippy::too_many_arguments)]
pub fn build_rg_args(
    mut vis: Visibility,
    sort: SortOrder,
    context: [usize; 2],
    case: When,
    no_heading: bool,
    fixed_strings: bool,
    patterns: &[String],
    paths: &[PathBuf],
    rg_args: &[OsString],
    cfg: &RgConfig,
) -> Vec<OsString> {
    if patterns.is_empty() && cfg.empty_pattern.is_none() {
        return vec!["".into()]; // empty command to fail fast
    }
    let mut ret: Vec<OsString> = vec![];
    // Initialize extra args
    // Add base args and user/default args
    let mut extra_args: Vec<OsString> = cfg.base_args.iter().map(|s| s.into()).collect();
    if rg_args.is_empty() {
        extra_args.extend(cfg.default_args.iter().map(|s| s.into()));
    }

    vis.no_follow |= cfg.base_args.iter().any(|x| x == "--no-follow");
    if !vis.no_follow {
        ret.push("--follow".into());
    }

    if vis.all() {
        ret.push("-uuu".into());
    } else {
        if vis.hidden || vis.hidden_only {
            ret.push("--hidden".into());
        }
        if !vis.ignore {
            ret.push("--no-ignore".into());
        }
    }

    match sort {
        SortOrder::mtime => {
            ret.push("--sortr=modified".into());
        }
        SortOrder::name => {
            ret.push("--sort=path".into());
        }
        // SortOrder::none => {
        //     ret.push("--sort=none".into());
        // }
        _ => {}
    }

    // add --iglob
    if !vis.all() {
        let iglobs = {
            let mut exclusions = cfg.iglobs.get(&PathBuf::new()).cloned().unwrap_or_default();

            // todo: check full/replaced for all paths
            if __cwd() == __home() {
                if let Some(excls) = cfg.iglobs.get(&PathBuf::from("~")) {
                    exclusions.extend(excls.iter().cloned());
                }
            } else if let Ok(stripped) = __cwd().strip_prefix(__home())
                && let Some(excls) = cfg.iglobs.get(&PathBuf::from("~").join(stripped))
            {
                exclusions.extend(excls.iter().cloned());
            } else if let Some(excls) = cfg.iglobs.get(__cwd()) {
                exclusions.extend(excls.iter().cloned());
            }

            exclusions
        };

        for e in iglobs {
            ret.push("--iglob".into());
            ret.push(e.into());
        }
    }

    for p in patterns {
        ret.push("--regexp".into());
        ret.push(p.into());
    }
    if patterns.is_empty() {
        let mut pat = cfg.empty_pattern.as_deref().unwrap();
        if let Some(stripped) = pat.strip_prefix("-v ") {
            ret.push("-v".into());
            pat = stripped;
        }
        ret.extend(["--regexp".into(), pat.into()]);
    }

    ret.append(&mut vec_![
    OsString:
    "--field-context-separator=:",
    "--line-number",
    "--column",
    if no_heading {
        "--no-heading"
    } else {
        "--heading"
    },
    "--null",
    "--hyperlink-format="
    ]);

    let case = match case {
        When::Never => "--ignore-case",
        When::Auto => "--smart-case",
        When::Always => "--case-sensitive",
    };
    ret.push(case.into());

    let fixed = if fixed_strings && !(patterns.is_empty()) {
        "--fixed-strings"
    } else {
        "--no-fixed-strings"
    };
    ret.push(fixed.into());

    ret.append(&mut vec_![: "--before-context", context[0].to_string(), "--after-context", context[1].to_string()]);

    ret.append(&mut extra_args);
    ret.extend_from_slice(rg_args);

    // rg [OPTIONS] -e PATTERN ... [PATH ...]
    ret.extend(paths.iter().map(Into::into));

    ret
}
