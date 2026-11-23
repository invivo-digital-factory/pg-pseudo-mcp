use crate::contexts::querying::domain::types::ColumnPolicyLoad;

pub trait ColumnPolicySource: Send + Sync {
    fn load(&self) -> ColumnPolicyLoad;
}
