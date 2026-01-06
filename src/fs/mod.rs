use std::{
    ffi::OsStr,
    fs, io,
    path::{MAIN_SEPARATOR, Path},
};

use cli_boilerplate_automation::bath::PathExt;

use crate::abspath::AbsPath;

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
pub fn create_all(files: &[Result<AbsPath, AbsPath>]) -> Result<(), io::Error> {
    for entry in files {
        match entry {
            Ok(file) => {
                if let Some(parent) = file.as_path().parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::File::create(file.as_path())?;
            }
            Err(dir) => {
                fs::create_dir_all(dir.as_path())?;
            }
        }
    }
    Ok(())
}

pub fn rename(
    src: &AbsPath,
    dst: &AbsPath,
) -> Result<(), io::Error> {
    if let Some(parent) = dst.as_path().parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(src.as_path(), dst.as_path())
}
