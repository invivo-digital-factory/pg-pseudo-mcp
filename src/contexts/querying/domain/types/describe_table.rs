use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeTableRequest {
    pub connection: String,
    pub database: Option<String>,
    pub table: String,
}

#[derive(Debug, Serialize)]
pub struct DescribeResponse {
    pub schema: String,
    pub name: String,
    pub kind: &'static str,
    pub estimated_rows: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub columns: Vec<ColumnEntry>,
    pub primary_key: Vec<String>,
    pub foreign_keys: Vec<ForeignKey>,
    pub referenced_by: Vec<Reference>,
    pub indexes: Vec<IndexEntry>,
}

#[derive(Debug, Serialize)]
pub struct ColumnEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub not_null: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudonymized: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ForeignKey {
    pub columns: Vec<String>,
    pub references: String,
}

#[derive(Debug, Serialize)]
pub struct Reference {
    pub table: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct IndexEntry {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub partial: bool,
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::querying::domain::constants::catalog::DETECTED_PER_VALUE;

    #[test]
    fn la_description_d_une_table_de_soixante_colonnes_tient_sous_quatre_ko() {
        let types = [
            "integer",
            "text",
            "varchar(80)",
            "timestamptz",
            "numeric(12,2)",
            "boolean",
        ];
        let columns = (0..60)
            .map(|index| ColumnEntry {
                name: format!("column_nb_{index:02}"),
                type_name: types[index % types.len()].to_string(),
                not_null: index % 3 == 0,
                pseudonymized: (index % types.len() == 1).then(|| DETECTED_PER_VALUE.to_string()),
                comment: None,
            })
            .collect();
        let response = DescribeResponse {
            schema: "public".into(),
            name: "wide_table".into(),
            kind: "table",
            estimated_rows: Some(120_000),
            comment: None,
            columns,
            primary_key: vec!["column_nb_00".into()],
            foreign_keys: vec![ForeignKey {
                columns: vec!["column_nb_02".into()],
                references: "public.customers(id)".into(),
            }],
            referenced_by: Vec::new(),
            indexes: vec![IndexEntry {
                name: "wide_table_pkey".into(),
                columns: vec!["column_nb_00".into()],
                unique: true,
                partial: false,
            }],
        };
        let size = serde_json::to_string(&response).unwrap().len();
        assert!(size < 4 * 1024, "{size} bytes");
    }
}
