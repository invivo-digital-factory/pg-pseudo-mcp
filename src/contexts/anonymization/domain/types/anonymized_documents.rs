use std::collections::BTreeMap;

use super::{ColumnFlag, Node};

pub struct AnonymizedDocuments {
    pub documents: Vec<Node>,
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
    pub omitted_paths: usize,
}
