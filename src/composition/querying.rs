use std::sync::Arc;

use super::adapters::{
    ColumnPolicySourceAdapter, PersonalColumnDictionaryAdapter, PseudonymTranslatorAdapter,
    ReadOnlyDatabaseAdapter, ResultAnonymizerAdapter,
};
use crate::contexts::anonymization::injector::AnonymizationUseCases;
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::repositories::AnonymizationPorts;
use crate::contexts::querying::injector::QueryingContext;

pub fn querying_context(
    connections: Arc<ConnectionsContext>,
    anonymization: Result<AnonymizationUseCases, String>,
) -> QueryingContext<ReadOnlyDatabaseAdapter> {
    let ports = anonymization.map(|use_cases| AnonymizationPorts {
        translator: Arc::new(PseudonymTranslatorAdapter::new(
            use_cases.translate_pseudonym.clone(),
        )),
        dictionary: Arc::new(PersonalColumnDictionaryAdapter::new(
            use_cases.dictionary_kind,
            use_cases.classify_column,
        )),
        anonymizer: Arc::new(ResultAnonymizerAdapter::new(use_cases)),
    });
    let policy = Arc::new(ColumnPolicySourceAdapter::new(Arc::clone(&connections)));
    QueryingContext::new(
        Arc::new(ReadOnlyDatabaseAdapter::new(connections)),
        ports,
        policy,
    )
}
