use super::FieldSource;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MongoFieldPattern {
    pub database: Option<String>,
    pub collection: Option<String>,
    pub path: String,
}

impl MongoFieldPattern {
    pub fn specificity(&self) -> u8 {
        match (&self.database, &self.collection) {
            (Some(_), Some(_)) => 4,
            (None, Some(_)) => 3,
            (Some(_), None) => 2,
            (None, None) => 1,
        }
    }

    pub fn matches(&self, source: &FieldSource) -> bool {
        self.path == source.path.to_lowercase() && self.covers_collection(source)
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
}
