use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListDatabasesRequest {
    pub connection: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DatabaseListing {
    pub databases: Vec<String>,
    pub default: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}
