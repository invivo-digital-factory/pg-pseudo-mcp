use super::cell::Cell;
use super::db_message::DbMessage;

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
