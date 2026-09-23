use std::{ffi::OsStr, path::PathBuf};

pub const VCS_DIRS: &[&str] = &[
    ".git", ".jj", ".sl", ".hg", ".svn", ".bzr", "_darcs", ".pijul", "CVS",
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

pub fn any_path_is_ignored<P: AsRef<std::path::Path>>(paths: impl IntoIterator<Item = P>) -> bool {
    let mut cmd = std::process::Command::new("git");
    cmd.arg("check-ignore").arg("--");
    let mut has_paths = false;
    for p in paths {
        let p_ref = p.as_ref();
        if p_ref.as_os_str().is_empty() {
            continue;
        }
        cmd.arg(p_ref);
        if p_ref.is_dir() {
            cmd.arg(format!("{}/", p_ref.display()));
        }
        has_paths = true;
    }
    if !has_paths {
        return false;
    }
    cmd.output()
        .map(|out| out.status.success() && !out.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_any_path_is_ignored() {
        assert!(any_path_is_ignored(["fist.log", "debug/"]));
        assert!(!any_path_is_ignored(["src"]));
        assert!(!any_path_is_ignored::<&str>([]));
    }
}


