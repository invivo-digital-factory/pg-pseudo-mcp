use crate::contexts::connections::domain::types::ColumnPolicyFile;

pub trait ColumnPolicyFiles: Send + Sync {
    fn read(&self) -> Vec<ColumnPolicyFile>;
}
