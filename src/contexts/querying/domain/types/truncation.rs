use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TruncatedBy {
    Rows,
    Bytes,
    Cell,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Truncation {
    pub by: TruncatedBy,
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    pub hint: String,
}
