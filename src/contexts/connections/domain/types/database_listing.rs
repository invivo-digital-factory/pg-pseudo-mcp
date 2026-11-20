#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseListing {
    pub databases: Vec<String>,
    pub default: Option<String>,
    pub truncated: bool,
}
