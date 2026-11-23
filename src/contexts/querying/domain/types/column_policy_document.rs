use super::ColumnPolicyEntries;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnPolicyDocument {
    pub file: String,
    pub content: Result<ColumnPolicyEntries, String>,
}
