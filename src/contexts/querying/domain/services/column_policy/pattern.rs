use crate::contexts::querying::domain::types::ColumnPattern;

const ANY_TABLE: &str = "*";

pub fn parse_pattern(text: &str) -> Result<ColumnPattern, String> {
    let invalid = || {
        format!(
            "invalid column pattern `{text}`: write schema.table.column, table.column or *.column"
        )
    };
    let parts: Vec<String> = text
        .split('.')
        .map(|part| part.trim().to_lowercase())
        .collect();
    let named = |part: &String| !part.is_empty() && part != ANY_TABLE;
    match parts.as_slice() {
        [any, column] if any == ANY_TABLE && named(column) => Ok(ColumnPattern {
            schema: None,
            table: None,
            column: column.clone(),
        }),
        [table, column] if named(table) && named(column) => Ok(ColumnPattern {
            schema: None,
            table: Some(table.clone()),
            column: column.clone(),
        }),
        [schema, table, column] if named(schema) && named(table) && named(column) => {
            Ok(ColumnPattern {
                schema: Some(schema.clone()),
                table: Some(table.clone()),
                column: column.clone(),
            })
        }
        _ => Err(invalid()),
    }
}
