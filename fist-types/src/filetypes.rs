use std::path::Path;

pub use super::ft_arg::FileTypeArg;

#[derive(
    Debug,
    Clone,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::EnumMessage,
    Copy,
    PartialEq,
    Eq,
    std::hash::Hash,
)]
#[strum(serialize_all = "kebab-case")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize,))]
pub enum FileType {
    /// file
    #[strum(serialize = "f")]
    File,
    /// directory
    #[strum(serialize = "d")]
    Directory,
    /// symlink
    #[strum(serialize = "l")]
    Symlink,
    /// block device
    #[strum(serialize = "b")]
    BlockDevice,
    /// char device
    #[strum(serialize = "c")]
    CharDevice,
    /// executable
    #[strum(serialize = "x")]
    Executable,
    /// empty
    #[strum(serialize = "e")]
    Empty,
    /// socket
    #[strum(serialize = "s")]
    Socket,
    /// pipe
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
