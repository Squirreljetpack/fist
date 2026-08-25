use std::io;

use crate::abspath::AbsPath;
use tokio::fs;

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
