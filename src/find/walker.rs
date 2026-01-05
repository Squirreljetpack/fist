use std::path::{Path, PathBuf};

use cli_boilerplate_automation::{bait::ResultExt, bath::PathExt};
use ignore::{
    WalkBuilder,
    overrides::{Override, OverrideBuilder},
};

use crate::filters::Visibility;

// paths are relative to root
pub fn list_dir(
    cwd: &Path,
    mut vis: Visibility,
    depth: usize,
) -> impl Iterator<Item = PathBuf> {
    let mut builder = WalkBuilder::new(cwd);
    if vis.all() {
        vis.hidden = true;
        vis.ignore = false;
    }
    builder
        .standard_filters(true)
        .git_ignore(vis.ignore)
        .git_global(vis.ignore)
        .git_exclude(vis.ignore)
        .require_git(false)
        .max_depth(Some(1));

    // hidden handling
    if vis.hidden_files {
        // ignore `hidden`, handle filtering manually
        builder.hidden(false);
    } else {
        builder.hidden(!vis.hidden);
    }

    let walker = builder.build();

    walker
        .filter_map(|e| e.ok())
        .filter(move |e| e.path() != cwd)
        .filter(move |e| {
            let path = e.path();
            let is_hidden = path.is_hidden();
            let is_dir = path.is_dir();

            if vis.hidden_files {
                // non-hidden always allowed
                if !is_hidden {
                    return true;
                }

                !(vis.dirs ^ is_dir)
            } else if vis.dirs {
                // normal behavior: dirs flag only filters dirs
                is_dir
            } else if vis.files {
                !is_dir
            } else {
                true
            }
        })
        .map(|e| e.into_path())
}

pub fn build_overrides<'a>(
    paths: &[&'a str],
    exclusions: impl IntoIterator<Item = &'a str>,
) -> Result<Override, String> {
    let mut builder = OverrideBuilder::new(paths[0]); // no absolute patterns

    for pattern in exclusions {
        builder.add(pattern).prefix("Malformed exclude pattern")?;
    }

    builder.build().prefix("Malformed exclude pattern")
}
