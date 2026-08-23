use std::fs::{self, File};
use std::io;
use std::path::Path;

#[cfg(unix)]
pub(crate) fn same_device(
    src: &Path,
    dst_parent: &Path,
) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::symlink_metadata(src), fs::metadata(dst_parent)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => false,
    }
}

#[cfg(not(unix))]
pub(crate) fn same_device(
    _src: &Path,
    _dst_parent: &Path,
) -> bool {
    false
}

pub(crate) fn clone_file(
    src: &Path,
    dst: &Path,
) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux_clone(src, dst)
    }
    #[cfg(target_os = "macos")]
    {
        macos_clone(src, dst)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (src, dst);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "reflink not supported on this platform",
        ))
    }
}

#[cfg(target_os = "linux")]
fn linux_clone(
    src: &Path,
    dst: &Path,
) -> io::Result<()> {
    use std::{fs::OpenOptions, os::unix::io::AsRawFd};
    let s = File::open(src)?;
    let d = OpenOptions::new().write(true).create_new(true).open(dst)?;
    let rc = unsafe { libc::ioctl(d.as_raw_fd(), libc::FICLONE, s.as_raw_fd()) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    drop(d);
    let _ = fs::remove_file(dst);
    Err(err)
}

#[cfg(target_os = "macos")]
fn macos_clone(
    src: &Path,
    dst: &Path,
) -> io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::AsRawFd;

    unsafe extern "C" {
        fn fclonefileat(
            src_fd: libc::c_int,
            dst_dirfd: libc::c_int,
            dst_name: *const libc::c_char,
            flags: libc::c_int,
        ) -> libc::c_int;
    }

    let name = dst
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "no file name"))?;
    let cname = CString::new(name.as_bytes())?;
    let parent = dst.parent().unwrap_or_else(|| Path::new("."));
    let dir = File::open(parent)?;
    let s = File::open(src)?;
    let rc = unsafe { fclonefileat(s.as_raw_fd(), dir.as_raw_fd(), cname.as_ptr(), 0) };
    if rc == 0 {
        return Ok(());
    }
    Err(io::Error::last_os_error())
}
