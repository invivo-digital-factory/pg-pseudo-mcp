use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::sqlite_pseudonym_store::companion_files;
use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::repositories::MappingPurger;
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::{
    SECRET_FILE, STORE_FILE,
};

pub struct FileMappingPurger {
    data_dir: PathBuf,
}

impl FileMappingPurger {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }
}

impl MappingPurger for FileMappingPurger {
    fn purge(&self) -> Result<Vec<PathBuf>, PseudoError> {
        let store_path = self.data_dir.join(STORE_FILE);
        let [wal, shm] = companion_files(&store_path);
        let mut deleted = Vec::new();
        for path in [store_path, wal, shm, self.data_dir.join(SECRET_FILE)] {
            match fs::remove_file(&path) {
                Ok(()) => deleted.push(path),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(source) => return Err(PseudoError::io("delete", &path, source)),
            }
        }
        Ok(deleted)
    }
}
