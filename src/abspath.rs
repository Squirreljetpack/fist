use std::{
    ffi::{OsStr, OsString},
    fmt,
    path::{Path, PathBuf},
};

use cli_boilerplate_automation::{bath::PathExt, impl_restricted_wrapper};

use crate::cli::paths;

impl_restricted_wrapper!(AbsPath, PathBuf, paths::__cwd().into());

impl AbsPath {
    /// Normalize + resolve paths relative to cwd
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into().abs(paths::__cwd());
        Self(path)
    }

    pub fn new_unchecked(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }

    pub fn to_os_string(&self) -> OsString {
        self.0.clone().into_os_string()
    }

    /// Since AbsPath is normalized, parent only fails if on root, in which case the sensible fallback is itself
    pub fn _parent(self) -> AbsPath {
        Path::parent(&self)
            .map(AbsPath::new_unchecked)
            .unwrap_or(self)
    }
}

impl From<AbsPath> for OsString {
    fn from(val: AbsPath) -> Self {
        val.0.into_os_string()
    }
}

impl AsRef<Path> for AbsPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<OsStr> for AbsPath {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref()
    }
}

impl fmt::Display for AbsPath {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        self.0.display().fmt(f)
    }
}
