use super::catalog_cells::{float, int, text};
use super::relation::{estimate, relation_kind, short_type};
use super::table_picker::Candidate;
use crate::contexts::querying::domain::types::{QueryOutput, SchemaMatch, TableEntry};

pub fn schema_matches(output: &QueryOutput) -> Vec<SchemaMatch> {
    output
        .rows
        .iter()
        .map(|row| SchemaMatch {
            schema: text(&row[0]).unwrap_or_default(),
            table: text(&row[1]).unwrap_or_default(),
            column: text(&row[2]),
            type_name: text(&row[3]).map(|type_name| short_type(&type_name)),
        })
        .collect()
}

pub fn table_entries(output: &QueryOutput) -> Vec<TableEntry> {
    output
        .rows
        .iter()
        .map(|row| {
            let kind = relation_kind(&text(&row[2]).unwrap_or_default());
            TableEntry {
                schema: text(&row[0]).unwrap_or_default(),
                name: text(&row[1]).unwrap_or_default(),
                kind,
                estimated_rows: estimate(kind, float(&row[3])),
                columns: int(&row[4]).unwrap_or(0),
            }
        })
        .collect()
}

pub fn candidates(output: &QueryOutput) -> Vec<Candidate> {
    output
        .rows
        .iter()
        .map(|row| Candidate {
            oid: int(&row[0]).unwrap_or_default(),
            schema: text(&row[1]).unwrap_or_default(),
            name: text(&row[2]).unwrap_or_default(),
            path_rank: int(&row[3]),
        })
        .collect()
}

pub fn known_tables(output: &QueryOutput) -> Vec<(String, String)> {
    output
        .rows
        .iter()
        .map(|row| {
            (
                text(&row[0]).unwrap_or_default(),
                text(&row[1]).unwrap_or_default(),
            )
        })
        .collect()
}
