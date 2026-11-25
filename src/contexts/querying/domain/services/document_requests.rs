use std::sync::Arc;

use crate::contexts::querying::domain::constants::mongo_policy::DATABASE_REQUIRED;
use crate::contexts::querying::domain::errors::{
    DatabaseFailure, QueryingError, Refusal, ScrubbedText,
};
use crate::contexts::querying::domain::repositories::{
    ColumnPolicySource, DocumentPorts, ReadOnlyDocumentStore,
};
use crate::contexts::querying::domain::types::{ColumnPolicy, QueryLimits, Translations};

pub fn ports_of(
    anonymization: &Result<DocumentPorts, String>,
) -> Result<&DocumentPorts, QueryingError> {
    anonymization
        .as_ref()
        .map_err(|reason| QueryingError::StoreUnavailable {
            reason: reason.clone(),
        })
}

pub fn policy_of(source: Option<&Arc<dyn ColumnPolicySource>>) -> ColumnPolicy {
    source
        .map(|source| source.load().policy)
        .unwrap_or_default()
}

pub async fn resolve_database<S: ReadOnlyDocumentStore>(
    store: &S,
    ports: &DocumentPorts,
    connection: &str,
    requested: Option<&str>,
) -> Result<String, QueryingError> {
    if let Some(database) = requested.filter(|name| !name.trim().is_empty()) {
        return Ok(database.to_string());
    }
    store
        .default_database(connection)
        .await
        .map_err(|failure| scrubbed(ports, failure, QueryLimits::default().timeout_ms, &[]))?
        .ok_or_else(|| {
            QueryingError::Refused(Refusal::new("mongo_database_required", DATABASE_REQUIRED))
        })
}

pub fn translations_for(ports: &DocumentPorts, literals: Vec<String>) -> Translations {
    let mut translations = Translations::default();
    for literal in literals {
        translations.record(&literal, ports.translator.lookup(&literal));
    }
    translations
}

pub fn scrubbed(
    ports: &DocumentPorts,
    failure: DatabaseFailure,
    timeout_ms: u64,
    injected: &[(String, String)],
) -> QueryingError {
    let scrubbed = match &failure {
        DatabaseFailure::Db(message) => {
            return QueryingError::DocumentServerError {
                message: ports.anonymizer.server_message(&message.message, injected),
                code_name: message.sqlstate.clone(),
            };
        }
        DatabaseFailure::Connect(text)
        | DatabaseFailure::Decode(text)
        | DatabaseFailure::ConnectFailed { reason: text, .. } => {
            ScrubbedText::Text(ports.anonymizer.free_text(text, injected))
        }
        _ => ScrubbedText::Nothing,
    };
    QueryingError::from_failure(failure, timeout_ms, scrubbed)
}
