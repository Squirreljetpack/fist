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
