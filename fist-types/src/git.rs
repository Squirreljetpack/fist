use std::{ffi::OsStr, path::PathBuf};

pub const VCS_DIRS: &[&str] = &[
    ".git",
    ".jj",
    ".sl",
    ".hg",
    ".svn",
    ".bzr",
    "_darcs",
    ".pijul",
    "CVS",
];

pub fn is_vcs_dir(name: impl AsRef<OsStr>) -> bool {
    let name = name.as_ref();
    VCS_DIRS.iter().any(|&vcs| name == vcs)
}

pub fn in_git_repo(mut dir: Option<PathBuf>) -> bool {
    while let Some(path) = dir {
        if path.join(".git").exists() {
            return true;
        }

        dir = path.parent().map(|p| p.to_path_buf());
    }

    false
}
