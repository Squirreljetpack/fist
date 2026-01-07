use std::{
    ffi::OsStr,
    io,
    path::{MAIN_SEPARATOR, Path},
};

use crate::abspath::AbsPath;
use cli_boilerplate_automation::bath::PathExt;
use tokio::fs;

// Returns Err(dest) if the given ends with slash
pub fn auto_dest(
    dest: impl AsRef<OsStr>,
    cwd: &Path,
) -> Result<AbsPath, AbsPath> {
    let dest = dest.as_ref();

    if dest.to_string_lossy().ends_with(MAIN_SEPARATOR) {
        let ret = AbsPath::new_unchecked(Path::new(dest).abs(cwd));
        Err(ret)
    } else {
        let ret = AbsPath::new_unchecked(Path::new(dest).abs(cwd));
        Ok(ret)
    }
}

// fails fast
pub async fn create_all(files: &[Result<AbsPath, AbsPath>]) -> Result<(), io::Error> {
    for entry in files {
        match entry {
            Ok(file) => {
                if let Some(parent) = file.as_path().parent() {
                    fs::create_dir_all(parent).await?;
                }
                fs::File::create(file.as_path()).await?;
            }
            Err(dir) => {
                fs::create_dir_all(dir.as_path()).await?;
            }
        }
    }
    Ok(())
}

pub async fn rename(
    src: &AbsPath,
    dst: &AbsPath,
) -> Result<(), io::Error> {
    if let Some(parent) = dst.as_path().parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::rename(src.as_path(), dst.as_path()).await
}
