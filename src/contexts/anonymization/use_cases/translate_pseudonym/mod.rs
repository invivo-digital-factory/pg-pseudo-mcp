use std::sync::Arc;

use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::PseudonymTranslation;

#[derive(Clone)]
pub struct TranslatePseudonym {
    pseudonymizer: Arc<Pseudonymizer>,
}

impl TranslatePseudonym {
    pub fn new(pseudonymizer: Arc<Pseudonymizer>) -> Self {
        Self { pseudonymizer }
    }

    pub fn execute(&self, text: &str) -> PseudonymTranslation {
        PseudonymTranslation {
            real: self.pseudonymizer.reverse(text),
            recognised: self.pseudonymizer.looks_like_pseudonym(text),
        }
    }
}
