pub fn in_git_repo(mut dir: Option<std::path::PathBuf>) -> bool {
    while let Some(path) = dir {
        if path.join(".git").exists() {
            return true;
        }

        dir = path.parent().map(|p| p.to_path_buf());
    }

    false
}
