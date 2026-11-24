use super::FieldSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldLineage {
    Source(FieldSource),
    Derived {
        inputs: Vec<FieldSource>,
        operators: Vec<String>,
    },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineageClearance {
    Source(FieldSource),
    Derived { proven_clear: bool },
}
