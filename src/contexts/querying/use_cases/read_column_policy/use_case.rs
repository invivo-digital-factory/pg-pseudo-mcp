use std::sync::Arc;

use crate::contexts::querying::domain::repositories::ColumnPolicySource;
use crate::contexts::querying::domain::types::ColumnPolicyLoad;

pub struct ReadColumnPolicy {
    source: Arc<dyn ColumnPolicySource>,
}

impl ReadColumnPolicy {
    pub fn new(source: Arc<dyn ColumnPolicySource>) -> Self {
        Self { source }
    }

    pub fn execute(&self) -> ColumnPolicyLoad {
        self.source.load()
    }
}
