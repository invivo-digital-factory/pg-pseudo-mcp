use toml::{Table, Value};

use crate::contexts::connections::domain::types::ColumnPolicyEntries;
use crate::contexts::connections::infrastructure::data_sources::connection_file::position_of;

const CLEAR_SHAPE: &str = "`clear` must be an array of column patterns";
const PERSONAL_SHAPE: &str =
    "`personal` must be a table, written [personal], of column patterns to kinds";

pub fn parse_policy(contents: &str) -> Result<ColumnPolicyEntries, String> {
    let table: Table = toml::from_str(contents).map_err(|error| {
        let position = position_of(contents, error.span().map_or(0, |span| span.start));
        format!(
            "syntax error at line {}, column {}",
            position.line, position.column
        )
    })?;
    let mut entries = ColumnPolicyEntries::default();
    for (key, value) in &table {
        match (key.as_str(), value) {
            ("clear", Value::Array(items)) => {
                for item in items {
                    let Value::String(pattern) = item else {
                        return Err(CLEAR_SHAPE.to_string());
                    };
                    entries.clear.push(pattern.clone());
                }
            }
            ("personal", Value::Table(kinds)) => {
                for (pattern, kind) in kinds {
                    let Value::String(kind) = kind else {
                        return Err(format!("the kind of `{pattern}` must be a string"));
                    };
                    entries.personal.push((pattern.clone(), kind.clone()));
                }
            }
            ("clear", _) => return Err(CLEAR_SHAPE.to_string()),
            ("personal", _) => return Err(PERSONAL_SHAPE.to_string()),
            (other, _) => return Err(format!("unknown top-level key `{other}`")),
        }
    }
    Ok(entries)
}
