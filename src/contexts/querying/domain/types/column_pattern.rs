use super::ColumnSource;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnPattern {
    pub schema: Option<String>,
    pub table: Option<String>,
    pub column: String,
}

impl ColumnPattern {
    pub fn specificity(&self) -> u8 {
        match (&self.schema, &self.table) {
            (Some(_), Some(_)) => 3,
            (None, Some(_)) => 2,
            _ => 1,
        }
    }

    pub fn matches(&self, source: &ColumnSource) -> bool {
        let same = |expected: &str, actual: &str| expected == actual.to_lowercase();
        same(&self.column, &source.column)
            && self
                .table
                .as_deref()
                .is_none_or(|table| same(table, &source.table))
            && self
                .schema
                .as_deref()
                .is_none_or(|schema| same(schema, &source.schema))
    }
}
