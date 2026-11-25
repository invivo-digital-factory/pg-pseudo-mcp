use std::sync::Arc;

use super::{PersonalColumnDictionary, PseudonymTranslator};
use crate::contexts::querying::domain::types::{
    AnonymizedDocuments, DocumentClearance, DocumentNode,
};

pub trait DocumentAnonymizer: Send + Sync {
    fn documents(
        &self,
        documents: Vec<DocumentNode>,
        clearances: &[(Vec<String>, DocumentClearance)],
        injected: &[(String, String)],
    ) -> AnonymizedDocuments;

    fn key(&self, key: &str) -> String;

    fn server_message(&self, text: &str, injected: &[(String, String)]) -> String;

    fn free_text(&self, text: &str, injected: &[(String, String)]) -> String;
}

#[derive(Clone)]
pub struct DocumentPorts {
    pub translator: Arc<dyn PseudonymTranslator>,
    pub anonymizer: Arc<dyn DocumentAnonymizer>,
    pub dictionary: Arc<dyn PersonalColumnDictionary>,
}
