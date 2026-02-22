use std::path::{Path, PathBuf};

use cli_boilerplate_automation::{StringError, bait::ResultExt};
use ignore::{
    WalkBuilder,
    overrides::{Override, OverrideBuilder},
};

use fist_types::filters::Visibility;

// paths are relative to root
pub fn list_dir(
    cwd: &Path,
    mut vis: Visibility,
    _depth: usize, // todo
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
    if vis.hidden || vis.hidden_only {
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
            vis.filter(path)
        })
        .map(|e| e.into_path())
}

pub fn build_overrides<'a>(
    paths: &[&'a str],
    exclusions: impl IntoIterator<Item = &'a str>,
) -> Result<Override, StringError> {
    let mut builder = OverrideBuilder::new(paths[0]); // no absolute patterns

    for pattern in exclusions {
        builder.add(pattern).prefix("Malformed exclude pattern")?;
    }

    builder.build().prefix("Malformed exclude pattern")
}
