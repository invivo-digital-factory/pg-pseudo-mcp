use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AggregateRequest {
    pub connection: String,
    pub database: Option<String>,
    pub collection: String,
    pub pipeline: Vec<Value>,
    pub let_vars: Option<Value>,
    pub max_rows: Option<u64>,
    pub max_bytes: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub cell_offset: Option<u64>,
    pub confirm_heavy: bool,
}
