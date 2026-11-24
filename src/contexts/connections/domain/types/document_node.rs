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
    Uuid(String),
    Timestamp(String),
    MinKey,
    MaxKey,
    Binary { subtype: u8, len: usize },
    Opaque { type_name: String, len: usize },
    Array(Vec<DocumentNode>),
    Object(Vec<(String, DocumentNode)>),
}
