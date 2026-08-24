use std::path::{Path, PathBuf};

use cba::{StringError, bait::ResultExt};
use ignore::{
    WalkBuilder,
    overrides::{Override, OverrideBuilder},
};

use fist_types::{filters::Visibility, git::is_vcs_dir};

// paths are relative to root
pub fn list_dir(
    cwd: &Path,
    mut vis: Visibility,
    _depth: usize, // todo
    ignore_patterns: &[String],
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
        // show `hidden`, handle filtering manually
        builder.hidden(false);
    } else {
        builder.hidden(!vis.hidden);
    }

    // configured ignore globs apply unless everything was requested
    if !vis.all() && !ignore_patterns.is_empty() {
        let mut overrides = OverrideBuilder::new(cwd);
        for pattern in ignore_patterns {
            if let Err(e) = overrides.add(&format!("!{pattern}")) {
                log::warn!("Invalid nav ignore pattern {pattern:?}: {e}");
            }
        }
        match overrides.build() {
            Ok(matcher) => {
                builder.overrides(matcher);
            }
            Err(e) => log::warn!("Failed to build nav ignore overrides: {e}"),
        }
    }

    let walker = builder.build();

    walker
        .filter_map(|e| e.ok())
        .filter(move |e| e.path() != cwd)
        .filter(move |e| {
            if vis.ignore && !vis.all() && is_vcs_dir(e.file_name()) {
                return false;
            }
            let path = e.path();
            vis.post_nav_filter(path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    fn listed_names(
        dir: &Path,
        ignore_patterns: &[&str],
        all: bool,
    ) -> Vec<String> {
        let mut vis = Visibility::DEFAULT;
        if all {
            vis.set_all(true);
        }
        let patterns: Vec<String> = ignore_patterns.iter().map(|s| (*s).into()).collect();
        let mut names: Vec<String> = list_dir(dir, vis, 1, &patterns)
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn test_ignore_patterns_apply_unless_all() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("target")).unwrap();
        File::create(temp.path().join("target").join("x.txt")).unwrap();
        File::create(temp.path().join("keep.txt")).unwrap();

        assert!(listed_names(temp.path(), &[], false).contains(&"target".into()));

        let names = listed_names(temp.path(), &["target"], false);
        assert!(!names.contains(&"target".into()));
        assert!(names.contains(&"keep.txt".into()));

        // visibility.all disables the ignore patterns entirely
        assert!(listed_names(temp.path(), &["target"], true).contains(&"target".into()));
    }
}
