use std::path::Path;

pub use super::ft_arg::FileTypeArg;

#[derive(
    Debug,
    Clone,
    strum_macros::Display,
    strum_macros::EnumString,
    Copy,
    PartialEq,
    Eq,
    std::hash::Hash,
)]
#[strum(serialize_all = "kebab-case")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize,))]
pub enum FileType {
    #[strum(serialize = "f")]
    File,
    #[strum(serialize = "d")]
    Directory,
    #[strum(serialize = "l")]
    Symlink,
    #[strum(serialize = "b")]
    BlockDevice,
    #[strum(serialize = "c")]
    CharDevice,
    #[strum(serialize = "x")]
    Executable,
    #[strum(serialize = "e")]
    Empty,
    #[strum(serialize = "s")]
    Socket,
    #[strum(serialize = "p")]
    Pipe,
}

impl FileType {
    pub fn get(path: &Path) -> Self {
        // query without following symlink
        let meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(_) => return FileType::File,
        };

        let ft = meta.file_type();

        if ft.is_symlink() {
            return FileType::Symlink;
        }
        if ft.is_dir() {
            return FileType::Directory;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;

            if ft.is_block_device() {
                return FileType::BlockDevice;
            }
            if ft.is_char_device() {
                return FileType::CharDevice;
            }
            if ft.is_socket() {
                return FileType::Socket;
            }
            if ft.is_fifo() {
                return FileType::Pipe;
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 != 0 {
                return FileType::Executable;
            }
        }
        #[cfg(windows)]
        {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "exe" | "bat" | "cmd" | "com") {
                return FileType::Executable;
            }
        }
        #[cfg(not(any(windows, unix)))]
        {
            return FileType::File;
        }

        FileType::File
    }
}
