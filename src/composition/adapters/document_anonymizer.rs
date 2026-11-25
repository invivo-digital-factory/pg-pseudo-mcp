use super::anonymization_mapping::flag;
use super::document_mapping::{field_clearance, node_in, node_out};
use crate::contexts::anonymization::domain::types::FieldPath;
use crate::contexts::anonymization::injector::AnonymizationUseCases;
use crate::contexts::querying::domain::constants::mongo_policy;
use crate::contexts::querying::domain::repositories::DocumentAnonymizer;
use crate::contexts::querying::domain::types::{
    AnonymizedDocuments, DocumentClearance, DocumentNode,
};

fn known_names() -> Vec<&'static str> {
    [
        mongo_policy::STAGES,
        mongo_policy::TOP_LEVEL_QUERY_OPERATORS,
        mongo_policy::FIELD_QUERY_OPERATORS,
        mongo_policy::GEOMETRY_OPERATORS,
        mongo_policy::TEXT_OPERATORS,
        mongo_policy::EXPRESSION_OPERATORS,
    ]
    .concat()
}

pub struct DocumentAnonymizerAdapter {
    use_cases: AnonymizationUseCases,
}

impl DocumentAnonymizerAdapter {
    pub fn new(use_cases: AnonymizationUseCases) -> Self {
        Self { use_cases }
    }
}

impl DocumentAnonymizer for DocumentAnonymizerAdapter {
    fn documents(
        &self,
        documents: Vec<DocumentNode>,
        clearances: &[(Vec<String>, DocumentClearance)],
        injected: &[(String, String)],
    ) -> AnonymizedDocuments {
        let clearances: Vec<_> = clearances
            .iter()
            .map(|(path, clearance)| {
                (
                    FieldPath::from_segments(path.clone()),
                    field_clearance(clearance),
                )
            })
            .collect();
        let anonymized = self.use_cases.anonymize_documents.execute(
            documents.into_iter().map(node_in).collect(),
            &clearances,
            injected,
        );
        AnonymizedDocuments {
            documents: anonymized.documents.into_iter().map(node_out).collect(),
            pseudonymized: anonymized
                .pseudonymized
                .into_iter()
                .map(|(path, column_flag)| (path, flag(column_flag)))
                .collect(),
            omitted_paths: anonymized.omitted_paths,
        }
    }

    fn key(&self, key: &str) -> String {
        self.use_cases.anonymize_documents.key(key)
    }

    fn server_message(&self, text: &str, injected: &[(String, String)]) -> String {
        self.use_cases
            .anonymize_documents
            .server_message(text, injected, &known_names())
    }

    fn free_text(&self, text: &str, injected: &[(String, String)]) -> String {
        self.use_cases.anonymize_free_text.execute(text, injected)
    }
}
