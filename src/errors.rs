use std::path::PathBuf;

use cli_boilerplate_automation::StringError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Invalid Path: {0}")]
    InvalidPath(PathBuf),
    #[error("Migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

#[non_exhaustive]
#[derive(Default, Debug, Error)]
pub enum CliError {
    #[default]
    #[error("Cli error")]
    Handled,
    #[error("Conflicting flags: --{0} and --{1}")]
    ConflictingFlags(&'static str, &'static str),
    #[error(transparent)]
    MatchError(#[from] matchmaker::MatchError),
    #[error(transparent)]
    DbError(#[from] DbError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    String(#[from] StringError),
}
