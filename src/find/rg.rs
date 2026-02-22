use std::{ffi::OsString, path::PathBuf};

use cli_boilerplate_automation::vec_;

use crate::{
    cli::paths::{__cwd, __home},
    config::RgConfig,
};
use fist_types::filters::Visibility;
use fist_types::When;

pub fn build_rg_args(
    mut vis: Visibility,
    paths: &[OsString],
    patterns: &[OsString],
    rg_args: &[OsString],
    case: When,
    context: [usize; 2],

    cfg: &RgConfig,
) -> Vec<OsString> {
    let mut ret = vec![];
    // Initialize extra args
    // Add base args and user/default args
    let mut extra_args: Vec<OsString> = cfg.base_args.iter().map(|s| s.into()).collect();

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
        ret.push(p.clone());
    }
    ret.append(&mut vec_![
        OsString:
        "--color=ansi",
        "--no-context-separator",
        "--field-context-separator=-",
        "--field-context-separator=:",
        "--line-number",
        "--column",
        "--heading"
    ]);

    ret.append(&mut extra_args);

    // rg [OPTIONS] -e PATTERN ... [PATH ...]
    ret.extend(paths.iter().cloned());

    ret
}
