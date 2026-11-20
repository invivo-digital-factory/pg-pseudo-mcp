#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub source: Option<ColumnSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnSource {
    pub schema: String,
    pub table: String,
    pub column: String,
}
