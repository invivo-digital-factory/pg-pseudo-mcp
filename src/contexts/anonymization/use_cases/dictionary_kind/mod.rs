use crate::contexts::anonymization::domain::services::dictionary::dictionary_kind;
use crate::contexts::anonymization::domain::types::Kind;

#[derive(Clone, Copy, Default)]
pub struct DictionaryKind;

impl DictionaryKind {
    pub fn execute(&self, column_name: &str) -> Option<Kind> {
        dictionary_kind(column_name)
    }
}
