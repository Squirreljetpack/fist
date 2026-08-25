use std::path::PathBuf;

use cba::claim::ClaimError;

use super::config::TransferParams;

#[derive(Debug, Clone)]
pub enum JobKind {
    /// A file/directory transfer; `TransferParams::r#move` picks copy vs
    /// move semantics (runtime-dynamic, driven by the queue row kind).
    Transfer(TransferParams),
    /// Unpack the source archive into `dest` (which must exist).
    Extract(ExtractParams),
}

/// Settings for an extraction job.
///
/// Progress denominators come from the extraction itself, which registers
/// each entry as it reaches it; this type is reserved for future options.
#[derive(Debug, Clone, Default)]
pub struct ExtractParams;

#[derive(Debug, Clone)]
pub struct JobRequest {
    pub kind: JobKind,
    pub source: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug)]
pub enum SubmitError {
    SourceMissing(PathBuf),
    IntoItself { source: PathBuf, dest: PathBuf },
    Claim(PathBuf, ClaimError),
    ShuttingDown,
}

impl std::fmt::Display for SubmitError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SubmitError::SourceMissing(p) => write!(f, "source does not exist: {}", p.display()),
            SubmitError::Claim(path, err) => {
                write!(f, "cannot use '{}': {}", path.display(), err)
            }
            SubmitError::IntoItself { source, dest } => {
                write!(
                    f,
                    "cannot copy directory into itself: {} -> {}",
                    source.display(),
                    dest.display()
                )
            }
            SubmitError::ShuttingDown => write!(f, "scheduler is shutting down"),
        }
    }
}

impl std::error::Error for SubmitError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SubmitError::Claim(_, err) => Some(err),
            _ => None,
        }
    }
}
