use crate::cli::paths::cwd;
use cli_boilerplate_automation::bath::PathExt;
use std::path::PathBuf;

pub fn paths_base<P>(paths: impl IntoIterator<Item = P>) -> PathBuf
where
    P: AsRef<std::path::Path>,
{
    let mut iter = paths.into_iter().map(|p| p.as_ref().abs(cwd())).peekable();

    let first = match iter.peek() {
        Some(p) => p.clone(),
        None => return PathBuf::new(),
    };

    let mut components: Vec<_> = first.components().collect();

    for path in iter {
        let mut i = 0;
        for (a, b) in components.iter().zip(path.components()) {
            if a != &b {
                break;
            }
            i += 1;
        }
        components.truncate(i);
        if components.is_empty() {
            break;
        }
    }

    components.iter().collect()
}
