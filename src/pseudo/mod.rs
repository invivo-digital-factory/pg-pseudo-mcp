mod generators;
mod secret;
mod store;

use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use unicode_normalization::UnicodeNormalization;

use crate::kind::Kind;
use crate::state::AppConfig;
use secret::Keys;
use store::{Outcome, Store};

const SECRET_FILE: &str = "secret";
const STORE_FILE: &str = "mapping.sqlite";
const MAX_DRAWS: usize = 64;
const PRIVATE_DIR_MODE: u32 = 0o700;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeneratorConfig {
    pub phone_pool_limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pseudonym {
    pub text: String,
    pub reversible: bool,
    pub exhausted: bool,
}

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
    Store(#[from] rusqlite::Error),
    #[error("system randomness unavailable: {0}")]
    Random(String),
    #[error("encryption of a mapping entry failed")]
    Crypto,
}

impl PseudoError {
    pub(crate) fn io(action: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }
}

pub struct Pseudonymizer {
    keys: Keys,
    store: Store,
    config: GeneratorConfig,
}

impl Pseudonymizer {
    pub fn open(data_dir: &Path) -> Result<Self, PseudoError> {
        Self::open_with_config(data_dir, GeneratorConfig::default())
    }

    pub fn open_default(config: &AppConfig) -> Result<Self, PseudoError> {
        Self::open(&data_dir(config)?)
    }

    pub fn open_with_config(data_dir: &Path, config: GeneratorConfig) -> Result<Self, PseudoError> {
        create_private_dir(data_dir)?;
        let secret_path = data_dir.join(SECRET_FILE);
        let store_path = data_dir.join(STORE_FILE);
        if store_path.exists() && !secret_path.exists() {
            return Err(PseudoError::MissingSecret {
                store: store_path,
                secret: secret_path,
            });
        }
        let keys = Keys::new(secret::load_or_create(&secret_path)?);
        let store = Store::open(&store_path, &keys.key_check())?;
        Ok(Self {
            keys,
            store,
            config,
        })
    }

    pub fn pseudonymize(&self, kind: Kind, value: &str) -> Pseudonym {
        if value.trim().is_empty() {
            return Pseudonym {
                text: value.to_string(),
                reversible: false,
                exhausted: false,
            };
        }
        let normalized: String = value.nfc().collect();
        let seed = self.keys.seed(kind, &normalized);
        let mut rng = ChaCha20Rng::from_seed(seed);
        if !kind.is_reversible() {
            return Pseudonym {
                text: generators::generate(kind, &normalized, &mut rng, &self.config),
                reversible: false,
                exhausted: false,
            };
        }
        let outcome = self.store.get_or_create(
            kind,
            &seed,
            MAX_DRAWS,
            || generators::generate(kind, &normalized, &mut rng, &self.config),
            || self.keys.encrypt(kind, &normalized),
        );
        match outcome {
            Ok(Outcome::Known(text) | Outcome::Created(text)) => Pseudonym {
                text,
                reversible: true,
                exhausted: false,
            },
            Ok(Outcome::Exhausted) => {
                tracing::warn!(
                    kind = kind.as_str(),
                    "pseudonym namespace exhausted; an opaque token replaces the value"
                );
                opaque(kind, &seed)
            }
            Err(error) => {
                tracing::warn!(
                    kind = kind.as_str(),
                    %error,
                    "pseudonym store unavailable; an opaque token replaces the value"
                );
                opaque(kind, &seed)
            }
        }
    }

    pub fn reverse(&self, text: &str) -> Option<(Kind, String)> {
        let rows = match self.store.find(text) {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(%error, "pseudonym store lookup failed");
                return None;
            }
        };
        rows.into_iter().find_map(|(name, ciphertext)| {
            let kind = Kind::parse(&name).filter(|kind| kind.is_reversible())?;
            self.keys
                .decrypt(kind, &ciphertext)
                .map(|value| (kind, value))
        })
    }

    pub fn looks_like_pseudonym(&self, text: &str) -> Option<Kind> {
        generators::recognise(text)
    }
}

fn opaque(kind: Kind, seed: &[u8; 32]) -> Pseudonym {
    Pseudonym {
        text: format!("<{} id={}>", kind.as_str(), hex::encode(&seed[..4])),
        reversible: false,
        exhausted: true,
    }
}

pub fn data_dir(config: &AppConfig) -> Result<PathBuf, PseudoError> {
    if let Some(dir) = &config.data_dir {
        return Ok(dir.clone());
    }
    directories::ProjectDirs::from("", "", "pg-pseudo-mcp")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or(PseudoError::NoDataDir)
}

pub fn purge(data_dir: &Path) -> Result<Vec<PathBuf>, PseudoError> {
    let store_path = data_dir.join(STORE_FILE);
    let [wal, shm] = store::companion_files(&store_path);
    let mut deleted = Vec::new();
    for path in [store_path, wal, shm, data_dir.join(SECRET_FILE)] {
        match fs::remove_file(&path) {
            Ok(()) => deleted.push(path),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(source) => return Err(PseudoError::io("delete", &path, source)),
        }
    }
    Ok(deleted)
}

fn create_private_dir(path: &Path) -> Result<(), PseudoError> {
    DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(path)
        .map_err(|source| PseudoError::io("create", path, source))
}
