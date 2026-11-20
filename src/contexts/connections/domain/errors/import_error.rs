use super::connection_file_write_error::ConnectionFileWriteError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImportError {
    #[error("{0}")]
    Source(String),
    #[error("DBeaver has no PostgreSQL connection to import")]
    NothingToImport,
    #[error(transparent)]
    File(#[from] ConnectionFileWriteError),
}
