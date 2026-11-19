use std::fs::DirBuilder;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};

use super::secret_file;
use crate::common::AppConfig;
use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::services::keys::Keys;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::GeneratorConfig;
use crate::contexts::anonymization::infrastructure::repositories::sqlite_pseudonym_store::SqlitePseudonymStore;

pub const SECRET_FILE: &str = "secret";
pub const STORE_FILE: &str = "mapping.sqlite";
const PRIVATE_DIR_MODE: u32 = 0o700;

pub fn data_dir(config: &AppConfig) -> Result<PathBuf, PseudoError> {
    if let Some(dir) = &config.data_dir {
        return Ok(dir.clone());
    }
    directories::ProjectDirs::from("", "", "pg-pseudo-mcp")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .ok_or(PseudoError::NoDataDir)
}

pub fn open_pseudonymizer(
    data_dir: &Path,
    config: GeneratorConfig,
) -> Result<Pseudonymizer, PseudoError> {
    create_private_dir(data_dir)?;
    let secret_path = data_dir.join(SECRET_FILE);
    let store_path = data_dir.join(STORE_FILE);
    if store_path.exists() && !secret_path.exists() {
        return Err(PseudoError::MissingSecret {
            store: store_path,
            secret: secret_path,
        });
    }
    let keys = Keys::new(secret_file::load_or_create(&secret_path)?);
    let store = SqlitePseudonymStore::open(&store_path, &keys.key_check())?;
    Ok(Pseudonymizer::new(keys, Box::new(store), config))
}

fn create_private_dir(path: &Path) -> Result<(), PseudoError> {
    DirBuilder::new()
        .recursive(true)
        .mode(PRIVATE_DIR_MODE)
        .create(path)
        .map_err(|source| PseudoError::io("create", path, source))
}
