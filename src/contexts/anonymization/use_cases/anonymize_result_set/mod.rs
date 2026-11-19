use std::sync::Arc;

use crate::contexts::anonymization::domain::services::anonymizer::Anonymizer;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::{AnonymizedRows, Cell, ColumnMeta, Lineage};

#[derive(Clone)]
pub struct AnonymizeResultSet {
    pseudonymizer: Arc<Pseudonymizer>,
}

impl AnonymizeResultSet {
    pub fn new(pseudonymizer: Arc<Pseudonymizer>) -> Self {
        Self { pseudonymizer }
    }

    pub fn execute(
        &self,
        columns: &[ColumnMeta],
        rows: Vec<Vec<Cell>>,
        lineage: Option<&[Lineage]>,
        injected: &[(String, String)],
    ) -> AnonymizedRows {
        Anonymizer::new(&self.pseudonymizer, injected).rows(columns, rows, lineage)
    }
}
