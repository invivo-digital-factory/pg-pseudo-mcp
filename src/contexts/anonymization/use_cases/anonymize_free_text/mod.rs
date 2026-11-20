use std::sync::Arc;

use crate::contexts::anonymization::domain::services::anonymizer::Anonymizer;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;

#[derive(Clone)]
pub struct AnonymizeFreeText {
    pseudonymizer: Arc<Pseudonymizer>,
}

impl AnonymizeFreeText {
    pub fn new(pseudonymizer: Arc<Pseudonymizer>) -> Self {
        Self { pseudonymizer }
    }

    pub fn execute(&self, text: &str, injected: &[(String, String)]) -> String {
        Anonymizer::new(&self.pseudonymizer, injected).free_text(text)
    }
}
