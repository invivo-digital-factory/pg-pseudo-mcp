#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionKind {
    Collection,
    View,
    Timeseries,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseNames {
    pub authorized: Vec<String>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionInfo {
    pub name: String,
    pub kind: CollectionKind,
    pub estimated_count: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexKeys {
    pub keys: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionDescription {
    pub estimated_count: Option<u64>,
    pub indexes: Vec<IndexKeys>,
    pub validator_fields: Vec<String>,
}
