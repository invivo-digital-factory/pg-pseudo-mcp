use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct FindRequest {
    pub database: Option<String>,
    pub collection: String,
    pub filter: Value,
    pub projection: Option<Value>,
    pub sort: Option<Value>,
    pub skip: Option<u64>,
    pub limit: Option<u64>,
    pub hint: Option<Value>,
    pub collation: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregateRequest {
    pub database: Option<String>,
    pub collection: String,
    pub pipeline: Vec<Value>,
    pub let_vars: Option<Value>,
}
