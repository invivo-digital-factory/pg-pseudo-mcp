#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseMessage {
    pub severity: String,
    pub sqlstate: Option<String>,
    pub message: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
    pub where_: Option<String>,
}
