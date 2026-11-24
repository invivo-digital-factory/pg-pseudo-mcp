use serde_json::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FindRequest {
    pub connection: String,
    pub database: Option<String>,
    pub collection: String,
    pub filter: Option<Value>,
    pub projection: Option<Value>,
    pub sort: Option<Value>,
    pub skip: Option<u64>,
    pub limit: Option<u64>,
    pub hint: Option<Value>,
    pub collation: Option<Value>,
    pub max_rows: Option<u64>,
    pub max_bytes: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub cell_offset: Option<u64>,
}
