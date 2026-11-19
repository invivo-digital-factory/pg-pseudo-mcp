use super::error::DbMessage;

#[derive(Debug, Clone, PartialEq)]
pub struct QueryOutput {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Cell>>,
    pub more_rows: bool,
    pub notices: Vec<DbMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub type_oid: u32,
    pub source: Option<ColumnSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnSource {
    pub schema: String,
    pub table: String,
    pub column: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Numeric(String),
    Text(String),
    Json(serde_json::Value),
    Uuid(String),
    Date(String),
    Time(String),
    Timestamp(String),
    TimestampTz(String),
    Interval(String),
    Inet(String),
    Array(Vec<Cell>),
    Composite(Vec<(String, Cell)>),
    Opaque { type_name: String, len: usize },
}
