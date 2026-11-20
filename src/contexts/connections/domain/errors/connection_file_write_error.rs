use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConnectionFileWriteError {
    #[error("the connection file location cannot be determined: no home directory found")]
    NoLocation,
    #[error("{} already exists; use --force to replace it", .0.display())]
    AlreadyExists(PathBuf),
    #[error("cannot write {}: {reason}", path.display())]
    Io { path: PathBuf, reason: String },
}
