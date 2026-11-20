use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListTablesRequest {
    pub connection: String,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub name_like: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ListResponse {
    pub tables: Vec<TableEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct TableEntry {
    pub schema: String,
    pub name: String,
    pub kind: &'static str,
    pub estimated_rows: Option<i64>,
    pub columns: i64,
}
