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
    Uuid { subtype: u8, value: String },
    Timestamp(String),
    MinKey,
    MaxKey,
    Binary { subtype: u8, len: usize },
    Opaque { type_name: String, len: usize },
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FieldPath(Vec<String>);

impl FieldPath {
    pub fn new(path: &str) -> Self {
        Self(
            path.split('.')
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect(),
        )
    }

    pub fn from_segments(segments: Vec<String>) -> Self {
        Self(segments)
    }

    pub fn child(&self, key: &str) -> Self {
        let mut segments = self.0.clone();
        segments.push(key.to_string());
        Self(segments)
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }

    pub fn dotted(&self) -> String {
        self.0.join(".")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldClearance {
    Clear,
    Personal(Kind),
    Derived { proven_clear: bool },
    NamedAs(String),
}
