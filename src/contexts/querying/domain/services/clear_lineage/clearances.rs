use sqlparser::ast::Statement;

use super::projection::ProjectionLineage;
use crate::contexts::querying::domain::types::{CatalogColumn, Clearance, RelationCatalog};

pub fn clearances(
    statement: &Statement,
    catalog: &RelationCatalog,
    is_clear_source: &dyn Fn(&CatalogColumn) -> bool,
    column_count: usize,
) -> Vec<Clearance> {
    let unproven = vec![Clearance::Unproven; column_count];
    let Statement::Query(query) = statement else {
        return unproven;
    };
    let lineage = ProjectionLineage {
        catalog,
        is_clear_source,
    };
    match lineage.columns(query) {
        Some(columns) if columns.len() == column_count => columns
            .iter()
            .map(|column| {
                if column.clear {
                    Clearance::ProvenClear
                } else {
                    Clearance::Unproven
                }
            })
            .collect(),
        _ => unproven,
    }
}
