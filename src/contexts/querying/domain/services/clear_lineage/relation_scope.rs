use sqlparser::ast::Ident;

use super::names::normalized;
use crate::contexts::querying::domain::types::{CatalogColumn, DerivedColumn};

pub enum ScopeRelation<'a> {
    Catalog {
        name: String,
        columns: &'a [CatalogColumn],
    },
    Derived {
        name: Option<String>,
        columns: Vec<DerivedColumn>,
    },
    Opaque,
}

enum Lookup {
    Found(bool),
    Missing,
    Unknown,
}

pub struct RelationScope<'a> {
    relations: Vec<ScopeRelation<'a>>,
    is_clear_source: &'a dyn Fn(&CatalogColumn) -> bool,
}

impl<'a> RelationScope<'a> {
    pub fn new(
        relations: Vec<ScopeRelation<'a>>,
        is_clear_source: &'a dyn Fn(&CatalogColumn) -> bool,
    ) -> Self {
        Self {
            relations,
            is_clear_source,
        }
    }

    pub fn is_clear_reference(&self, parts: &[Ident]) -> bool {
        match parts {
            [column] => {
                let column = normalized(column);
                let mut found = Vec::new();
                for relation in &self.relations {
                    match self.lookup(relation, &column) {
                        Lookup::Found(clear) => found.push(clear),
                        Lookup::Missing => {}
                        Lookup::Unknown => return false,
                    }
                }
                found == [true]
            }
            [relation, column] => {
                let relation = normalized(relation);
                let column = normalized(column);
                let named: Vec<&ScopeRelation<'_>> = self
                    .relations
                    .iter()
                    .filter(|candidate| name_of(candidate) == Some(relation.as_str()))
                    .collect();
                matches!(named.as_slice(), [only] if matches!(self.lookup(only, &column), Lookup::Found(true)))
            }
            _ => false,
        }
    }

    fn lookup(&self, relation: &ScopeRelation<'_>, column: &str) -> Lookup {
        match relation {
            ScopeRelation::Catalog { columns, .. } => {
                let matching: Vec<&CatalogColumn> = columns
                    .iter()
                    .filter(|candidate| candidate.name == column)
                    .collect();
                match matching.as_slice() {
                    [] => Lookup::Missing,
                    [only] => Lookup::Found((self.is_clear_source)(only)),
                    _ => Lookup::Unknown,
                }
            }
            ScopeRelation::Derived { columns, .. } => {
                let matching: Vec<&DerivedColumn> = columns
                    .iter()
                    .filter(|candidate| candidate.name.as_deref() == Some(column))
                    .collect();
                match matching.as_slice() {
                    [] => Lookup::Missing,
                    [only] => Lookup::Found(only.clear),
                    _ => Lookup::Unknown,
                }
            }
            ScopeRelation::Opaque => Lookup::Unknown,
        }
    }
}

fn name_of<'b>(relation: &'b ScopeRelation<'_>) -> Option<&'b str> {
    match relation {
        ScopeRelation::Catalog { name, .. } => Some(name),
        ScopeRelation::Derived { name, .. } => name.as_deref(),
        ScopeRelation::Opaque => None,
    }
}
