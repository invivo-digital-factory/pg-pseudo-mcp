#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clearance {
    ProvenClear,
    ClearByPolicy,
    PersonalByPolicy(String),
    Unproven,
}
