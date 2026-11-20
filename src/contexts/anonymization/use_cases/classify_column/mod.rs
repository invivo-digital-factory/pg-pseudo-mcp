use crate::contexts::anonymization::domain::services::classify::classify_column;
use crate::contexts::anonymization::domain::types::{ColumnClass, ColumnInfo};

#[derive(Clone, Copy, Default)]
pub struct ClassifyColumn;

impl ClassifyColumn {
    pub fn execute(&self, info: &ColumnInfo) -> ColumnClass {
        classify_column(info)
    }
}
