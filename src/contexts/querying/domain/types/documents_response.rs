use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::{ColumnFlag, FieldPolicyHint, Truncation, Warning};

#[derive(Debug, Serialize)]
pub struct DocumentsResponse {
    pub documents: Vec<Value>,
    pub document_count: usize,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub column_policy_hints: Vec<FieldPolicyHint>,
    pub translated_literals: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub truncated: Vec<Truncation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    pub elapsed_ms: u128,
}
