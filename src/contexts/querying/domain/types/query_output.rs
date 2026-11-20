use super::{Cell, ColumnMeta, DbMessage};

#[derive(Debug, Clone, PartialEq)]
pub struct QueryOutput {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<Cell>>,
    pub more_rows: bool,
    pub notices: Vec<DbMessage>,
}
