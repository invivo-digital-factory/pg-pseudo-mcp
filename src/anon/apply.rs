use serde_json::Value;

use super::convert;
use crate::contexts::anonymization::injector::AnonymizationUseCases;
use crate::guard::Lineage;
use crate::pg::{Cell, ColumnMeta, DbMessage};

pub use crate::contexts::anonymization::domain::types::{AnonymizedRows, ColumnFlag};

pub struct Anonymizer {
    use_cases: AnonymizationUseCases,
    injected: Vec<(String, String)>,
}

impl Anonymizer {
    pub fn new(use_cases: &AnonymizationUseCases, injected: &[(String, String)]) -> Self {
        Self {
            use_cases: use_cases.clone(),
            injected: injected.to_vec(),
        }
    }

    pub fn rows(
        &self,
        columns: &[ColumnMeta],
        rows: Vec<Vec<Cell>>,
        lineage: Option<&[Lineage]>,
    ) -> AnonymizedRows {
        let columns: Vec<_> = columns.iter().map(convert::column).collect();
        let rows = rows
            .into_iter()
            .map(|row| row.into_iter().map(convert::cell).collect())
            .collect();
        let lineage: Option<Vec<_>> =
            lineage.map(|items| items.iter().copied().map(convert::lineage).collect());
        self.use_cases.anonymize_result_set.execute(
            &columns,
            rows,
            lineage.as_deref(),
            &self.injected,
        )
    }

    pub fn free_text(&self, text: &str) -> String {
        self.use_cases
            .anonymize_free_text
            .execute(text, &self.injected)
    }

    pub fn message(&self, message: &DbMessage) -> DbMessage {
        convert::message_out(
            self.use_cases
                .anonymize_message
                .execute(&convert::message_in(message), &self.injected),
        )
    }

    pub fn plan(&self, plan: Value) -> Value {
        self.use_cases.anonymize_plan.execute(plan, &self.injected)
    }
}
