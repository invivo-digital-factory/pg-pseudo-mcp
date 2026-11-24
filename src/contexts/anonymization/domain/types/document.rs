use super::Kind;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
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
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FieldPath(String);

impl FieldPath {
    pub fn new(path: &str) -> Self {
        Self(path.to_string())
    }

    pub fn child(&self, key: &str) -> Self {
        if self.0.is_empty() {
            Self(key.to_string())
        } else {
            Self(format!("{}.{key}", self.0))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClearance {
    Clear,
    Personal(Kind),
    Derived { proven_clear: bool },
}
