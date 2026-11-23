use std::collections::HashMap;

use super::CatalogColumn;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RelationCatalog {
    relations: HashMap<String, Vec<CatalogColumn>>,
}

impl RelationCatalog {
    pub fn insert(&mut self, relation: &str, columns: Vec<CatalogColumn>) {
        self.relations.insert(relation.to_string(), columns);
    }

    pub fn columns(&self, relation: &str) -> Option<&[CatalogColumn]> {
        self.relations.get(relation).map(Vec::as_slice)
    }
}
