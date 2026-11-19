use std::collections::BTreeMap;

use serde_json::Value;

use super::ColumnFlag;

pub struct AnonymizedRows {
    pub rows: Vec<Vec<Value>>,
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
    pub per_column: Vec<Option<ColumnFlag>>,
}
