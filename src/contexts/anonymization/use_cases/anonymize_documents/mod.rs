use std::sync::Arc;

use crate::contexts::anonymization::domain::services::anonymizer::Anonymizer;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::{
    AnonymizedDocuments, FieldClearance, FieldPath, Node,
};

#[derive(Clone)]
pub struct AnonymizeDocuments {
    pseudonymizer: Arc<Pseudonymizer>,
}

impl AnonymizeDocuments {
    pub fn new(pseudonymizer: Arc<Pseudonymizer>) -> Self {
        Self { pseudonymizer }
    }

    pub fn execute(
        &self,
        documents: Vec<Node>,
        clearances: &[(FieldPath, FieldClearance)],
        injected: &[(String, String)],
    ) -> AnonymizedDocuments {
        Anonymizer::new(&self.pseudonymizer, injected).documents(documents, clearances)
    }
}
