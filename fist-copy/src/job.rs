use std::path::PathBuf;

use crate::config::{CopyParams, MoveParams};

#[derive(Debug, Clone)]
pub enum JobKind {
    Copy(CopyParams),
    Move(MoveParams),
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitError {
    SourceMissing(PathBuf),
    IntoItself { source: PathBuf, dest: PathBuf },
    ShuttingDown,
}

impl std::fmt::Display for SubmitError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            SubmitError::SourceMissing(p) => write!(f, "source does not exist: {}", p.display()),
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

impl std::error::Error for SubmitError {}
