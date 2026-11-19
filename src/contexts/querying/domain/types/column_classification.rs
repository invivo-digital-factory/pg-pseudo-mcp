#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnClassification {
    pub forced_kind: Option<String>,
    pub clear_by_type: bool,
}
