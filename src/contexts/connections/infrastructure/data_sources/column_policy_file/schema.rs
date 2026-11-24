use toml::{Table, Value};

use crate::contexts::connections::domain::types::{ColumnPolicyEntries, MongoPolicyEntries};
use crate::contexts::connections::infrastructure::data_sources::connection_file::position_of;

const CLEAR_SHAPE: &str = "`clear` must be an array of column patterns";
const PERSONAL_SHAPE: &str =
    "`personal` must be a table, written [personal], of column patterns to kinds";

const MONGODB_SHAPE: &str = "`mongodb` must be a table with `clear` and `personal` sections";
const MONGODB_CLEAR_SHAPE: &str =
    "`mongodb.clear` must be a table of collection keys to arrays of field paths";
const MONGODB_PERSONAL_SHAPE: &str =
    "`mongodb.personal` must hold one table per collection key, of field paths to kinds";

fn parse_mongodb(sections: &Table) -> Result<MongoPolicyEntries, String> {
    let mut entries = MongoPolicyEntries::default();
    for (section, value) in sections {
        match (section.as_str(), value) {
            ("clear", Value::Table(collections)) => {
                for (collection, paths) in collections {
                    let Value::Array(paths) = paths else {
                        return Err(MONGODB_CLEAR_SHAPE.to_string());
                    };
                    for path in paths {
                        let Value::String(path) = path else {
                            return Err(MONGODB_CLEAR_SHAPE.to_string());
                        };
                        entries.clear.push((collection.clone(), path.clone()));
                    }
                }
            }
            ("personal", Value::Table(collections)) => {
                for (collection, kinds) in collections {
                    let Value::Table(kinds) = kinds else {
                        return Err(MONGODB_PERSONAL_SHAPE.to_string());
                    };
                    for (path, kind) in kinds {
                        let Value::String(kind) = kind else {
                            return Err(format!(
                                "the kind of `{collection}` `{path}` must be a string"
                            ));
                        };
                        entries
                            .personal
                            .push((collection.clone(), path.clone(), kind.clone()));
                    }
                }
            }
            ("clear", _) => return Err(MONGODB_CLEAR_SHAPE.to_string()),
            ("personal", _) => return Err(MONGODB_PERSONAL_SHAPE.to_string()),
            (other, _) => return Err(format!("unknown key `mongodb.{other}`")),
        }
    }
    Ok(entries)
}

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
            ("mongodb", Value::Table(sections)) => entries.mongodb = parse_mongodb(sections)?,
            ("mongodb", _) => return Err(MONGODB_SHAPE.to_string()),
            ("clear", _) => return Err(CLEAR_SHAPE.to_string()),
            ("personal", _) => return Err(PERSONAL_SHAPE.to_string()),
            (other, _) => return Err(format!("unknown top-level key `{other}`")),
        }
    }
    Ok(entries)
}
