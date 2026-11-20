use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchSchemaRequest {
    pub connection: String,
    pub database: Option<String>,
    pub pattern: String,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub matches: Vec<SchemaMatch>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

#[derive(Debug, Serialize)]
pub struct SchemaMatch {
    pub schema: String,
    pub table: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
}
