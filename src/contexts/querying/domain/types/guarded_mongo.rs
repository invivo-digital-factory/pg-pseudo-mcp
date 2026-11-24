use serde_json::Value;

use super::{ClampWarning, Warning};

#[derive(Debug, Clone, PartialEq)]
pub struct GuardedMongo {
    pub database: Option<String>,
    pub collection: String,
    pub command: MongoCommand,
    pub execution: MongoExecution,
    pub translated_literals: usize,
    pub injected: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
    pub clamps: Vec<ClampWarning>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MongoCommand {
    Find(Box<GuardedFind>),
    Aggregate(GuardedAggregate),
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuardedFind {
    pub filter: Value,
    pub projection: Option<Value>,
    pub sort: Option<Value>,
    pub skip: Option<u64>,
    pub limit: u64,
    pub hint: Option<Value>,
    pub collation: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuardedAggregate {
    pub pipeline: Vec<Value>,
    pub let_vars: Option<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MongoReadPreference {
    SecondaryPreferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MongoExecution {
    pub max_time_ms: u64,
    pub allow_disk_use: bool,
    pub read_preference: MongoReadPreference,
    pub batch_size: u32,
    pub max_documents: usize,
    pub comment: &'static str,
}
