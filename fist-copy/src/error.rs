use std::io;

#[derive(Debug)]
pub(crate) enum WorkError {
    Canceled,
    Io(io::Error),
}

impl From<io::Error> for WorkError {
    fn from(e: io::Error) -> Self {
        WorkError::Io(e)
    }
}

#[cfg(unix)]
pub(crate) fn is_cross_device(e: &io::Error) -> bool {
    e.raw_os_error() == Some(libc::EXDEV)
}

#[cfg(windows)]
pub(crate) fn is_cross_device(e: &io::Error) -> bool {
    const ERROR_NOT_SAME_DEVICE: i32 = 17;
    e.raw_os_error() == Some(ERROR_NOT_SAME_DEVICE)
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn is_cross_device(_e: &io::Error) -> bool {
    false
}
