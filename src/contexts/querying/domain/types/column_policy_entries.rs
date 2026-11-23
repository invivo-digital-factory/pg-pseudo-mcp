#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColumnPolicyEntries {
    pub clear: Vec<String>,
    pub personal: Vec<(String, String)>,
}
