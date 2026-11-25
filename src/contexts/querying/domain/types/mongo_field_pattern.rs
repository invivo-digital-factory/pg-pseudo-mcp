use std::cmp::Ordering;

use super::FieldSource;
use crate::contexts::querying::domain::constants::column_policy::ANY_FIELD_SEGMENT;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MongoFieldPattern {
    pub database: Option<String>,
    pub collection: Option<String>,
    pub path: String,
}

impl MongoFieldPattern {
    pub fn key_specificity(&self) -> u8 {
        match (&self.database, &self.collection) {
            (Some(_), Some(_)) => 4,
            (None, Some(_)) => 3,
            (Some(_), None) => 2,
            (None, None) => 1,
        }
    }

    pub fn named_segments(&self) -> usize {
        self.segments()
            .filter(|segment| *segment != ANY_FIELD_SEGMENT)
            .count()
    }

    pub fn precedence(&self, other: &MongoFieldPattern) -> Ordering {
        (self.key_specificity(), self.named_segments())
            .cmp(&(other.key_specificity(), other.named_segments()))
    }

    pub fn matches(&self, source: &FieldSource) -> bool {
        let path = source.path.to_lowercase();
        self.segments().count() == path.split('.').count()
            && self.covers_prefix(&path)
            && self.covers_collection(source)
    }

    pub fn lies_below(&self, source: &FieldSource) -> bool {
        let path = source.path.to_lowercase();
        self.segments().count() > path.split('.').count()
            && self.covers_prefix(&path)
            && self.covers_collection(source)
    }

    pub fn covers_collection(&self, source: &FieldSource) -> bool {
        self.database
            .as_deref()
            .is_none_or(|database| database == source.database)
            && self
                .collection
                .as_deref()
                .is_none_or(|collection| collection == source.collection)
    }

    fn segments(&self) -> std::str::Split<'_, char> {
        self.path.split('.')
    }

    fn covers_prefix(&self, path: &str) -> bool {
        self.segments()
            .zip(path.split('.'))
            .all(|(pattern, field)| pattern == ANY_FIELD_SEGMENT || pattern == field)
    }
}
