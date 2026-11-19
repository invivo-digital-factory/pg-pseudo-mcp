use std::sync::Arc;

use crate::contexts::anonymization::domain::services::anonymizer::Anonymizer;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::DatabaseMessage;

#[derive(Clone)]
pub struct AnonymizeMessage {
    pseudonymizer: Arc<Pseudonymizer>,
}

impl AnonymizeMessage {
    pub fn new(pseudonymizer: Arc<Pseudonymizer>) -> Self {
        Self { pseudonymizer }
    }

    pub fn execute(
        &self,
        message: &DatabaseMessage,
        injected: &[(String, String)],
    ) -> DatabaseMessage {
        Anonymizer::new(&self.pseudonymizer, injected).message(message)
    }
}
