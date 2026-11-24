use serde_json::{Map, Value};

use crate::contexts::querying::domain::constants::mongo_policy::{
    INVALID_DATABASE_CHARACTERS, MAX_ARRAY_ELEMENTS, MAX_NESTING_DEPTH, MAX_REQUEST_BYTES,
    SYSTEM_COLLECTION_PREFIX, SYSTEM_DATABASES, SYSTEM_NAMESPACE_HINT,
};
use crate::contexts::querying::domain::errors::Refusal;

pub(super) enum NameProblem {
    System,
    Invalid,
}

pub(super) fn check_namespace(database: Option<&str>, collection: &str) -> Result<(), Refusal> {
    if let Some(database) = database {
        if SYSTEM_DATABASES.contains(&database.to_lowercase().as_str()) {
            return Err(system_refusal("the database is a system database"));
        }
        if database.is_empty() || database.contains(INVALID_DATABASE_CHARACTERS) {
            return Err(Refusal::new(
                "mongo_invalid_name",
                "the database name is not a valid MongoDB database name",
            ));
        }
    }
    match collection_problem(collection) {
        Some(NameProblem::System) => Err(system_refusal("the collection is a system collection")),
        Some(NameProblem::Invalid) => Err(Refusal::new(
            "mongo_invalid_name",
            "the collection name is not a valid MongoDB collection name",
        )),
        None => Ok(()),
    }
}

pub(super) fn collection_problem(name: &str) -> Option<NameProblem> {
    if name.is_empty() || name.contains(['$', '\0']) {
        return Some(NameProblem::Invalid);
    }
    name.to_lowercase()
        .starts_with(SYSTEM_COLLECTION_PREFIX)
        .then_some(NameProblem::System)
}

pub(super) fn system_refusal(what: &str) -> Refusal {
    Refusal::new("mongo_system_namespace", what).with_hint(SYSTEM_NAMESPACE_HINT)
}

pub(super) fn check_request(root: &Map<String, Value>) -> Result<(), Refusal> {
    let mut pending: Vec<(&Value, usize)> = root.values().map(|part| (part, 1)).collect();
    while let Some((value, depth)) = pending.pop() {
        match value {
            Value::Array(items) => {
                if items.len() > MAX_ARRAY_ELEMENTS {
                    return Err(Refusal::new(
                        "mongo_array_too_long",
                        format!(
                            "an array of the request has more than {MAX_ARRAY_ELEMENTS} elements"
                        ),
                    ));
                }
                check_depth(depth)?;
                pending.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(entries) => {
                check_depth(depth)?;
                pending.extend(entries.values().map(|item| (item, depth + 1)));
            }
            _ => {}
        }
    }
    let size: usize = root.values().map(|part| part.to_string().len()).sum();
    if size > MAX_REQUEST_BYTES {
        return Err(Refusal::new(
            "mongo_request_too_large",
            format!("the request is larger than {MAX_REQUEST_BYTES} bytes of JSON"),
        )
        .with_hint("narrow the request; long value lists belong in several calls"));
    }
    Ok(())
}

fn check_depth(depth: usize) -> Result<(), Refusal> {
    if depth > MAX_NESTING_DEPTH {
        return Err(Refusal::new(
            "mongo_nesting_too_deep",
            format!("the request nests objects and arrays deeper than {MAX_NESTING_DEPTH} levels"),
        ));
    }
    Ok(())
}
