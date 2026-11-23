use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::contexts::querying::domain::errors::DatabaseFailure;
use crate::contexts::querying::domain::repositories::{
    AnonymizationPorts, PersonalColumnDictionary, PseudonymTranslator, ReadOnlyDatabase,
    ResultAnonymizer,
};
use crate::contexts::querying::domain::types::DatabaseListing;
use crate::contexts::querying::domain::types::{
    AnonymizedRows, Cell, ColumnClassification, ColumnMeta, DbMessage, ExecutionLimits, Lineage,
    PseudonymLookup, QueryOutput,
};

pub struct FakeDatabase {
    response: Result<QueryOutput, DatabaseFailure>,
    listing: Result<DatabaseListing, DatabaseFailure>,
    enum_labels: Result<Vec<String>, DatabaseFailure>,
    statements: Mutex<Vec<String>>,
}

impl FakeDatabase {
    pub fn returning(output: QueryOutput) -> Arc<Self> {
        Arc::new(Self {
            response: Ok(output),
            listing: Err(DatabaseFailure::MissingParams),
            enum_labels: Ok(Vec::new()),
            statements: Mutex::new(Vec::new()),
        })
    }

    pub fn returning_with_enum_labels(
        output: QueryOutput,
        enum_labels: Result<Vec<String>, DatabaseFailure>,
    ) -> Arc<Self> {
        Arc::new(Self {
            response: Ok(output),
            listing: Err(DatabaseFailure::MissingParams),
            enum_labels,
            statements: Mutex::new(Vec::new()),
        })
    }

    pub fn failing(failure: DatabaseFailure) -> Arc<Self> {
        Arc::new(Self {
            response: Err(failure.clone()),
            listing: Err(failure.clone()),
            enum_labels: Err(failure),
            statements: Mutex::new(Vec::new()),
        })
    }

    pub fn listing(listing: DatabaseListing) -> Arc<Self> {
        Arc::new(Self {
            response: Ok(empty_output()),
            listing: Ok(listing),
            enum_labels: Ok(Vec::new()),
            statements: Mutex::new(Vec::new()),
        })
    }

    pub fn statements(&self) -> Vec<String> {
        self.statements.lock().unwrap().clone()
    }

    fn answer(&self, sql: &str) -> Result<QueryOutput, DatabaseFailure> {
        self.statements.lock().unwrap().push(sql.to_string());
        self.response.clone()
    }
}

impl ReadOnlyDatabase for FakeDatabase {
    async fn run(
        &self,
        _connection: &str,
        _database: Option<&str>,
        sql: &str,
        _limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.answer(sql)
    }

    async fn explain(
        &self,
        _connection: &str,
        _database: Option<&str>,
        explain_sql: &str,
        _limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.answer(explain_sql)
    }

    async fn read_catalog(
        &self,
        _connection: &str,
        _database: Option<&str>,
        catalog_sql: &str,
        _limits: ExecutionLimits,
    ) -> Result<QueryOutput, DatabaseFailure> {
        self.answer(catalog_sql)
    }

    async fn list_databases(&self, _connection: &str) -> Result<DatabaseListing, DatabaseFailure> {
        self.listing.clone()
    }

    async fn enum_labels(
        &self,
        _connection: &str,
        _database: Option<&str>,
    ) -> Result<Vec<String>, DatabaseFailure> {
        self.enum_labels.clone()
    }
}

#[derive(Default)]
pub struct FakeTranslator {
    pseudonyms: HashMap<String, String>,
}

impl FakeTranslator {
    pub fn with(mut self, pseudonym: &str, real: &str) -> Self {
        self.pseudonyms
            .insert(pseudonym.to_string(), real.to_string());
        self
    }
}

impl PseudonymTranslator for FakeTranslator {
    fn lookup(&self, text: &str) -> PseudonymLookup {
        if let Some(real) = self.pseudonyms.get(text) {
            return PseudonymLookup::Known { real: real.clone() };
        }
        if text.ends_with("@example.net") {
            PseudonymLookup::Unknown
        } else {
            PseudonymLookup::NotAPseudonym
        }
    }
}

#[derive(Default)]
pub struct FakeAnonymizer {
    injected_seen: Mutex<Vec<Vec<(String, String)>>>,
    enum_labels_seen: Mutex<Vec<Vec<String>>>,
    masks: Vec<(String, String)>,
}

impl FakeAnonymizer {
    pub fn masking(real: &str, pseudonym: &str) -> Arc<Self> {
        Arc::new(Self {
            injected_seen: Mutex::default(),
            enum_labels_seen: Mutex::default(),
            masks: vec![(real.to_string(), pseudonym.to_string())],
        })
    }

    pub fn injected_seen(&self) -> Vec<Vec<(String, String)>> {
        self.injected_seen.lock().unwrap().clone()
    }

    pub fn enum_labels_seen(&self) -> Vec<Vec<String>> {
        self.enum_labels_seen.lock().unwrap().clone()
    }

    fn replace(&self, text: &str, injected: &[(String, String)]) -> String {
        self.injected_seen.lock().unwrap().push(injected.to_vec());
        injected
            .iter()
            .chain(self.masks.iter())
            .fold(text.to_string(), |text, (real, pseudonym)| {
                text.replace(real, pseudonym)
            })
    }
}

impl ResultAnonymizer for FakeAnonymizer {
    fn rows(
        &self,
        columns: &[ColumnMeta],
        rows: Vec<Vec<Cell>>,
        _lineage: Option<&[Lineage]>,
        injected: &[(String, String)],
        enum_labels: &[String],
    ) -> AnonymizedRows {
        self.injected_seen.lock().unwrap().push(injected.to_vec());
        self.enum_labels_seen
            .lock()
            .unwrap()
            .push(enum_labels.to_vec());
        AnonymizedRows {
            rows: rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| match cell {
                            Cell::Text(text) => Value::String(text),
                            Cell::Int(number) => Value::from(number),
                            _ => Value::Null,
                        })
                        .collect()
                })
                .collect(),
            pseudonymized: Default::default(),
            per_column: vec![None; columns.len()],
        }
    }

    fn free_text(&self, text: &str, injected: &[(String, String)]) -> String {
        self.replace(text, injected)
    }

    fn message(&self, message: &DbMessage, injected: &[(String, String)]) -> DbMessage {
        DbMessage {
            message: self.replace(&message.message, injected),
            detail: message
                .detail
                .as_deref()
                .map(|detail| self.replace(detail, injected)),
            hint: message
                .hint
                .as_deref()
                .map(|hint| self.replace(hint, injected)),
            ..message.clone()
        }
    }

    fn plan(&self, plan: Value, injected: &[(String, String)]) -> Value {
        self.injected_seen.lock().unwrap().push(injected.to_vec());
        plan
    }
}

pub struct FakeDictionary;

impl PersonalColumnDictionary for FakeDictionary {
    fn dictionary_kind(&self, column_name: &str) -> Option<String> {
        (column_name == "email").then(|| "email".to_string())
    }

    fn classify(&self, _column_name: &str, _type_name: &str) -> ColumnClassification {
        ColumnClassification {
            forced_kind: None,
            clear_by_type: false,
        }
    }
}

pub fn ports(
    translator: FakeTranslator,
    anonymizer: Arc<FakeAnonymizer>,
) -> Result<AnonymizationPorts, String> {
    Ok(AnonymizationPorts {
        translator: Arc::new(translator),
        anonymizer,
        dictionary: Arc::new(FakeDictionary),
    })
}

pub fn empty_output() -> QueryOutput {
    QueryOutput {
        columns: Vec::new(),
        rows: Vec::new(),
        more_rows: false,
        notices: Vec::new(),
    }
}

pub fn sql_error(message: &str) -> DatabaseFailure {
    DatabaseFailure::Db(Box::new(DbMessage {
        severity: "ERROR".into(),
        sqlstate: Some("22P02".into()),
        message: message.into(),
        detail: None,
        hint: None,
        where_: None,
    }))
}
