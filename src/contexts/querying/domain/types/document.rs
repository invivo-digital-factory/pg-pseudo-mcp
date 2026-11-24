use std::collections::BTreeMap;

use super::ColumnFlag;

#[derive(Debug, Clone, PartialEq)]
pub enum DocumentNode {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    Decimal(String),
    Text(String),
    Date(String),
    ObjectId(String),
    Uuid { subtype: u8, value: String },
    Timestamp(String),
    MinKey,
    MaxKey,
    Binary { subtype: u8, len: usize },
    Opaque { type_name: String, len: usize },
    Array(Vec<DocumentNode>),
    Object(Vec<(String, DocumentNode)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentOutput {
    pub documents: Vec<DocumentNode>,
    pub more_documents: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentClearance {
    Clear,
    Personal(String),
    Derived { proven_clear: bool },
    NamedAs(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnonymizedDocuments {
    pub documents: Vec<DocumentNode>,
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
    pub omitted_paths: usize,
}
