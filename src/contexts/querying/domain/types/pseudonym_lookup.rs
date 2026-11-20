#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudonymLookup {
    Known { real: String },
    Unknown,
    NotAPseudonym,
}
