use super::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clearance {
    ProvenClear,
    ClearByPolicy,
    PersonalByPolicy(Kind),
    Unproven,
}
