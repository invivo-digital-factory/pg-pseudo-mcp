use crate::contexts::querying::domain::types::ColumnClassification;

pub trait PersonalColumnDictionary: Send + Sync {
    fn dictionary_kind(&self, column_name: &str) -> Option<String>;

    fn classify(&self, column_name: &str, type_name: &str) -> ColumnClassification;
}
