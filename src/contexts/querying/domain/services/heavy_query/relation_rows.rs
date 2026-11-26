use std::collections::HashMap;

use crate::contexts::querying::domain::services::catalog_cells::{float, text};
use crate::contexts::querying::domain::services::sql_literal::literal;
use crate::contexts::querying::domain::types::{PlanSummary, QueryOutput};

pub fn relation_rows_sql(summary: &PlanSummary) -> Option<String> {
    let mut pairs: Vec<String> = summary
        .scans
        .iter()
        .filter_map(|scan| {
            let schema = scan.schema.as_deref()?;
            Some(format!(
                "({}, {})",
                literal(schema),
                literal(&scan.relation)
            ))
        })
        .collect();
    pairs.sort();
    pairs.dedup();
    if pairs.is_empty() {
        return None;
    }
    Some(format!(
        "SELECT n.nspname::text, c.relname::text, c.reltuples::float8 \
         FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
         WHERE (n.nspname::text, c.relname::text) IN ({})",
        pairs.join(", ")
    ))
}

pub fn relation_rows(output: &QueryOutput) -> HashMap<String, f64> {
    output
        .rows
        .iter()
        .filter_map(|row| {
            let schema = text(row.first()?)?;
            let relation = text(row.get(1)?)?;
            let rows = float(row.get(2)?)?;
            Some((format!("{schema}.{relation}"), rows))
        })
        .collect()
}
