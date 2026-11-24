use crate::contexts::querying::domain::types::{ColumnPattern, MongoFieldPattern};

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

fn scope(part: &str, invalid: &str) -> Result<Option<String>, String> {
    match part.trim() {
        "" => Err(invalid.to_string()),
        ANY_TABLE => Ok(None),
        name => Ok(Some(name.to_string())),
    }
}

pub fn parse_field_pattern(key: &str, path: &str) -> Result<MongoFieldPattern, String> {
    let invalid_key = format!(
        "invalid collection key `{key}`: write database.collection, *.collection, database.* or *.*"
    );
    let (database, collection) = key.split_once('.').ok_or_else(|| invalid_key.clone())?;
    let path = path.trim().to_lowercase();
    if path.split('.').any(str::is_empty) {
        return Err(format!("invalid field path `{path}` for `{key}`"));
    }
    Ok(MongoFieldPattern {
        database: scope(database, &invalid_key)?,
        collection: scope(collection, &invalid_key)?,
        path,
    })
}
