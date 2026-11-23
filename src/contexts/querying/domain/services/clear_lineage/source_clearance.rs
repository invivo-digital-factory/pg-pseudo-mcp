use crate::contexts::querying::domain::repositories::PersonalColumnDictionary;
use crate::contexts::querying::domain::types::{
    CatalogColumn, ColumnPolicy, ColumnSource, PolicyDecision,
};

pub fn is_clear_source(
    dictionary: &dyn PersonalColumnDictionary,
    policy: &ColumnPolicy,
    column: &CatalogColumn,
) -> bool {
    match policy.decide(&source_of(column)) {
        Some(PolicyDecision::Personal(_)) => false,
        _ => is_clear_by_type(dictionary, column),
    }
}

pub fn is_clear_source_by_policy(
    dictionary: &dyn PersonalColumnDictionary,
    policy: &ColumnPolicy,
    column: &CatalogColumn,
) -> bool {
    match policy.decide(&source_of(column)) {
        Some(PolicyDecision::Personal(_)) => false,
        Some(PolicyDecision::Clear) => true,
        None => is_clear_by_type(dictionary, column),
    }
}

fn is_clear_by_type(dictionary: &dyn PersonalColumnDictionary, column: &CatalogColumn) -> bool {
    dictionary.dictionary_kind(&column.name).is_none()
        && dictionary
            .classify(&column.name, &column.type_name)
            .clear_by_type
}

fn source_of(column: &CatalogColumn) -> ColumnSource {
    ColumnSource {
        schema: column.schema.clone(),
        table: column.table.clone(),
        column: column.name.clone(),
    }
}
