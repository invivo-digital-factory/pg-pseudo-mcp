use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::{ColumnFlag, Truncation, Warning};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueryRequest {
    pub connection: String,
    pub sql: String,
    pub max_rows: Option<u64>,
    pub max_bytes: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub cell_offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub columns: Vec<ColumnOut>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
    pub translated_literals: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub truncated: Vec<Truncation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<NoticeOut>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct ColumnOut {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudonymized: Option<ColumnFlag>,
}

#[derive(Debug, Serialize)]
pub struct NoticeOut {
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
