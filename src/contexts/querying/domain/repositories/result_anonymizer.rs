use serde_json::Value;

use crate::contexts::querying::domain::types::{
    AnonymizedRows, Cell, ColumnMeta, DbMessage, Lineage,
};

pub trait ResultAnonymizer: Send + Sync {
    fn rows(
        &self,
        columns: &[ColumnMeta],
        rows: Vec<Vec<Cell>>,
        lineage: Option<&[Lineage]>,
        injected: &[(String, String)],
        enum_labels: &[String],
    ) -> AnonymizedRows;

    fn free_text(&self, text: &str, injected: &[(String, String)]) -> String;

    fn message(&self, message: &DbMessage, injected: &[(String, String)]) -> DbMessage;

    fn plan(&self, plan: Value, injected: &[(String, String)]) -> Value;
}
