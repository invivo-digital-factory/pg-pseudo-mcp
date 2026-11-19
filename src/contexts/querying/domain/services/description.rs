use serde::Deserialize;

use super::catalog_cells::{float, json_cell, text};
use super::column_pseudonymization::pseudonymization;
use super::relation::{estimate, relation_kind, short_type};
use crate::contexts::querying::domain::types::{
    Cell, ColumnClassification, ColumnEntry, DescribeResponse, ForeignKey, IndexEntry, Reference,
};

#[derive(Debug, Deserialize)]
pub struct RawColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub not_null: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawForeignKey {
    columns: Vec<String>,
    ref_schema: String,
    ref_table: String,
    ref_columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawReference {
    schema: String,
    table: String,
    columns: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawIndex {
    name: String,
    unique: bool,
    partial: bool,
    columns: Vec<Option<String>>,
}

pub struct Description {
    schema: String,
    name: String,
    kind: &'static str,
    estimated_rows: Option<i64>,
    pub comment: Option<String>,
    columns: Vec<RawColumn>,
    primary_key: Vec<String>,
    foreign_keys: Vec<RawForeignKey>,
    referenced_by: Vec<RawReference>,
    indexes: Vec<RawIndex>,
}

pub fn read_description(row: &[Cell]) -> Description {
    let kind = relation_kind(&text(&row[2]).unwrap_or_default());
    Description {
        schema: text(&row[0]).unwrap_or_default(),
        name: text(&row[1]).unwrap_or_default(),
        kind,
        estimated_rows: estimate(kind, float(&row[3])),
        comment: text(&row[4]),
        columns: json_cell(&row[5]),
        primary_key: json_cell(&row[6]),
        foreign_keys: json_cell(&row[7]),
        referenced_by: json_cell(&row[8]),
        indexes: json_cell(&row[9]),
    }
}

pub fn column_entry(
    column: RawColumn,
    class: &ColumnClassification,
    comment: Option<String>,
) -> ColumnEntry {
    let type_name = short_type(&column.type_name);
    ColumnEntry {
        pseudonymized: pseudonymization(class, &type_name),
        name: column.name,
        type_name,
        not_null: column.not_null,
        comment,
    }
}

impl Description {
    pub fn take_columns(&mut self) -> Vec<RawColumn> {
        std::mem::take(&mut self.columns)
    }

    pub fn into_response(
        self,
        comment: Option<String>,
        columns: Vec<ColumnEntry>,
    ) -> DescribeResponse {
        DescribeResponse {
            schema: self.schema,
            name: self.name,
            kind: self.kind,
            estimated_rows: self.estimated_rows,
            comment,
            columns,
            primary_key: self.primary_key,
            foreign_keys: self
                .foreign_keys
                .into_iter()
                .map(|fk| ForeignKey {
                    references: format!(
                        "{}.{}({})",
                        fk.ref_schema,
                        fk.ref_table,
                        fk.ref_columns.join(", ")
                    ),
                    columns: fk.columns,
                })
                .collect(),
            referenced_by: self
                .referenced_by
                .into_iter()
                .map(|reference| Reference {
                    table: format!("{}.{}", reference.schema, reference.table),
                    columns: reference.columns,
                })
                .collect(),
            indexes: self
                .indexes
                .into_iter()
                .map(|index| IndexEntry {
                    name: index.name,
                    unique: index.unique,
                    partial: index.partial,
                    columns: index
                        .columns
                        .into_iter()
                        .map(|column| column.unwrap_or_else(|| "(expression)".into()))
                        .collect(),
                })
                .collect(),
        }
    }
}
