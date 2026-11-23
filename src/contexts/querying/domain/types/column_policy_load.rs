use super::ColumnPolicy;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnPolicyLoad {
    pub policy: ColumnPolicy,
    pub warnings: Vec<String>,
}
