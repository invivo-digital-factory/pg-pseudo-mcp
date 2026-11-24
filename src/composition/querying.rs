use std::sync::Arc;

use super::adapters::{
    ColumnPolicySourceAdapter, ConnectionEnginesAdapter, DocumentAnonymizerAdapter,
    PersonalColumnDictionaryAdapter, PseudonymTranslatorAdapter, ReadOnlyDatabaseAdapter,
    ReadOnlyDocumentStoreAdapter, ResultAnonymizerAdapter,
};
use crate::contexts::anonymization::injector::AnonymizationUseCases;
use crate::contexts::connections::injector::ConnectionsContext;
use crate::contexts::querying::domain::repositories::{AnonymizationPorts, DocumentPorts};
use crate::contexts::querying::injector::{Ports, QueryingContext, Stores};

pub type Querying = QueryingContext<
    ReadOnlyDatabaseAdapter,
    ReadOnlyDocumentStoreAdapter,
    ConnectionEnginesAdapter,
>;

pub fn querying_context(
    connections: Arc<ConnectionsContext>,
    anonymization: Result<AnonymizationUseCases, String>,
) -> Querying {
    let rows = anonymization.clone().map(|use_cases| AnonymizationPorts {
        translator: Arc::new(PseudonymTranslatorAdapter::new(
            use_cases.translate_pseudonym.clone(),
        )),
        dictionary: Arc::new(PersonalColumnDictionaryAdapter::new(
            use_cases.dictionary_kind,
            use_cases.classify_column,
        )),
        anonymizer: Arc::new(ResultAnonymizerAdapter::new(use_cases)),
    });
    let documents = anonymization.map(|use_cases| DocumentPorts {
        translator: Arc::new(PseudonymTranslatorAdapter::new(
            use_cases.translate_pseudonym.clone(),
        )),
        dictionary: Arc::new(PersonalColumnDictionaryAdapter::new(
            use_cases.dictionary_kind,
            use_cases.classify_column,
        )),
        anonymizer: Arc::new(DocumentAnonymizerAdapter::new(use_cases)),
    });
    let policy = Arc::new(ColumnPolicySourceAdapter::new(Arc::clone(&connections)));
    QueryingContext::new(
        Stores {
            database: Arc::new(ReadOnlyDatabaseAdapter::new(Arc::clone(&connections))),
            documents: Arc::new(ReadOnlyDocumentStoreAdapter::new(Arc::clone(&connections))),
            engines: Arc::new(ConnectionEnginesAdapter::new(connections)),
        },
        Ports { rows, documents },
        policy,
    )
}
