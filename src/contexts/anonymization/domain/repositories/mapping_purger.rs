use std::path::PathBuf;

use crate::contexts::anonymization::domain::errors::PseudoError;

pub trait MappingPurger: Send + Sync {
    fn purge(&self) -> Result<Vec<PathBuf>, PseudoError>;
}
