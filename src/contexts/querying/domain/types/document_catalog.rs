use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    PostgreSql,
    MongoDb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionEntry {
    pub name: String,
    pub kind: &'static str,
    pub estimated_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionDetails {
    pub estimated_count: Option<u64>,
    pub indexes: Vec<Vec<(String, String)>>,
    pub validator_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FieldSample {
    pub path: Vec<String>,
    pub types: Vec<String>,
    pub presence: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCollectionsRequest {
    pub connection: String,
    pub database: Option<String>,
    pub name_like: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CollectionListResponse {
    pub database: String,
    pub collections: Vec<CollectionLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CollectionLine {
    pub name: String,
    pub kind: &'static str,
    pub estimated_documents: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct CollectionDescribeResponse {
    pub database: String,
    pub collection: String,
    pub estimated_documents: Option<u64>,
    pub sampled_documents_hint: &'static str,
    pub fields: Vec<FieldEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub column_policy_hints: Vec<super::FieldPolicyHint>,
    pub indexes: Vec<Vec<IndexKey>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub validator_fields: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FieldEntry {
    pub path: String,
    pub types: Vec<String>,
    pub presence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudonymized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct IndexKey {
    pub field: String,
    pub order: String,
}
