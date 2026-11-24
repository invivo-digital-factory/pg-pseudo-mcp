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

const DIRECTORY_NAME: &str = "scarif-database-mcp";
const LEGACY_DIRECTORY_NAME: &str = "pg-pseudo-mcp";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirectory {
    pub path: PathBuf,
    pub preferred: PathBuf,
    pub legacy: bool,
}

pub fn data_dir(config: &AppConfig) -> Result<PathBuf, PseudoError> {
    resolve_data_dir(config).map(|directory| directory.path)
}

pub fn resolve_data_dir(config: &AppConfig) -> Result<DataDirectory, PseudoError> {
    if let Some(dir) = &config.data_dir {
        return Ok(DataDirectory {
            path: dir.clone(),
            preferred: dir.clone(),
            legacy: false,
        });
    }
    let named = |name: &str| {
        directories::ProjectDirs::from("", "", name).map(|dirs| dirs.data_dir().to_path_buf())
    };
    let preferred = named(DIRECTORY_NAME).ok_or(PseudoError::NoDataDir)?;
    let legacy = named(LEGACY_DIRECTORY_NAME).ok_or(PseudoError::NoDataDir)?;
    Ok(keep_existing_legacy(preferred, legacy))
}

pub fn keep_existing_legacy(preferred: PathBuf, legacy: PathBuf) -> DataDirectory {
    if !preferred.exists() && legacy.exists() {
        return DataDirectory {
            path: legacy,
            preferred,
            legacy: true,
        };
    }
    DataDirectory {
        path: preferred.clone(),
        preferred,
        legacy: false,
    }
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

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind};

    #[test]
    fn le_nouveau_dossier_est_utilise_quand_il_existe_meme_si_l_ancien_existe() {
        let root = tempfile::tempdir().unwrap();
        let preferred = root.path().join("scarif-database-mcp");
        let legacy = root.path().join("pg-pseudo-mcp");
        std::fs::create_dir(&preferred).unwrap();
        std::fs::create_dir(&legacy).unwrap();
        let resolved = keep_existing_legacy(preferred.clone(), legacy);
        assert_eq!(resolved.path, preferred);
        assert!(!resolved.legacy);
    }

    #[test]
    fn le_dossier_par_defaut_est_le_nouveau_quand_aucun_n_existe() {
        let root = tempfile::tempdir().unwrap();
        let preferred = root.path().join("scarif-database-mcp");
        let resolved = keep_existing_legacy(preferred.clone(), root.path().join("pg-pseudo-mcp"));
        assert_eq!(resolved.path, preferred);
        assert!(!resolved.legacy);
    }

    #[test]
    fn l_ancien_dossier_est_conserve_sans_rien_creer_ni_deplacer() {
        let root = tempfile::tempdir().unwrap();
        let preferred = root.path().join("scarif-database-mcp");
        let legacy = root.path().join("pg-pseudo-mcp");
        std::fs::create_dir(&legacy).unwrap();
        let resolved = keep_existing_legacy(preferred.clone(), legacy.clone());
        assert_eq!(resolved.path, legacy);
        assert_eq!(resolved.preferred, preferred);
        assert!(resolved.legacy);
        assert!(!preferred.exists());
    }

    #[test]
    fn un_pseudonyme_connu_reste_identique_en_reutilisant_l_ancien_dossier() {
        let root = tempfile::tempdir().unwrap();
        let preferred = root.path().join("scarif-database-mcp");
        let legacy = root.path().join("pg-pseudo-mcp");
        let first = open_pseudonymizer(&legacy, GeneratorConfig::default()).unwrap();
        let known = first.pseudonymize(Kind::Email, "alice@example.org").text;
        drop(first);
        let resolved = keep_existing_legacy(preferred.clone(), legacy);
        let again = open_pseudonymizer(&resolved.path, GeneratorConfig::default()).unwrap();
        assert_eq!(
            again.pseudonymize(Kind::Email, "alice@example.org").text,
            known
        );
        assert!(!preferred.exists());
    }

    #[test]
    fn un_dossier_explicite_n_est_jamais_remplace() {
        let config = AppConfig {
            data_dir: Some(PathBuf::from("/explicit")),
            ..AppConfig::default()
        };
        let resolved = resolve_data_dir(&config).unwrap();
        assert_eq!(resolved.path, PathBuf::from("/explicit"));
        assert!(!resolved.legacy);
    }
}
