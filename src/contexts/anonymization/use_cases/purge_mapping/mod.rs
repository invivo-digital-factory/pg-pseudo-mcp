use std::path::PathBuf;
use std::sync::Arc;

use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::repositories::MappingPurger;

#[derive(Clone)]
pub struct PurgeMapping {
    purger: Arc<dyn MappingPurger>,
}

impl PurgeMapping {
    pub fn new(purger: Arc<dyn MappingPurger>) -> Self {
        Self { purger }
    }

    pub fn execute(&self) -> Result<Vec<PathBuf>, PseudoError> {
        self.purger.purge()
    }
}
