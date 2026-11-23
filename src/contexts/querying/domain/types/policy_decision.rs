#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Clear,
    Personal(String),
}
