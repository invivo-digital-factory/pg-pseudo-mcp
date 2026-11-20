use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PseudoError {
    #[error("no data directory could be determined for pg-pseudo-mcp; set PG_PSEUDO_DATA_DIR")]
    NoDataDir,
    #[error("cannot {action} {}: {source}", path.display())]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "the pseudonym store {} exists but its secret {} is missing, so stored pseudonyms can no longer be translated back; run `pg-pseudo-mcp mapping purge` to start a new mapping",
        store.display(),
        secret.display()
    )]
    MissingSecret { store: PathBuf, secret: PathBuf },
    #[error(
        "the pseudonym secret {} is corrupt; run `pg-pseudo-mcp mapping purge` to start a new mapping",
        path.display()
    )]
    CorruptSecret { path: PathBuf },
    #[error(
        "the pseudonym store {} was created with another secret; run `pg-pseudo-mcp mapping purge` to start a new mapping",
        store.display()
    )]
    SecretMismatch { store: PathBuf },
    #[error("the pseudonym store {} cannot use WAL journaling (got {mode})", store.display())]
    WalUnavailable { store: PathBuf, mode: String },
    #[error("pseudonym store error: {0}")]
    Store(String),
    #[error("system randomness unavailable: {0}")]
    Random(String),
    #[error("encryption of a mapping entry failed")]
    Crypto,
}

impl PseudoError {
    pub fn io(action: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
}
