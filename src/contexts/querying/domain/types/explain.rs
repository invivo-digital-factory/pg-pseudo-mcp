use serde::Serialize;

use super::Warning;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainRequest {
    pub connection: String,
    pub sql: String,
}

#[derive(Debug, Serialize)]
pub struct ExplainResponse {
    pub plan: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    pub translated_literals: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<Warning>,
}
