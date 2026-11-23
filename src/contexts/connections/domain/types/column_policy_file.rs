use super::ColumnPolicyEntries;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnPolicyFile {
    pub file: String,
    pub content: Result<ColumnPolicyEntries, String>,
}
