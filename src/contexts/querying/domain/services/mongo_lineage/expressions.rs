use serde_json::{Map, Value};

use super::shape::{Derivation, Shape};
use crate::contexts::querying::domain::constants::clear_lineage::{
    CONSTANT_VARIABLES, FIELD_NAME_OPERATORS,
};
use crate::contexts::querying::domain::constants::mongo_policy::EXTENDED_JSON_TYPES;

pub fn evaluate(value: &Value, document: &Shape) -> Shape {
    match value {
        Value::String(text) => match text.strip_prefix('$') {
            Some(variable) if variable.starts_with('$') => self::variable(&variable[1..]),
            Some(path) => match field_path(path) {
                Some(path) => document.at(&path),
                None => Shape::unknown(),
            },
            None => Shape::constant(),
        },
        Value::Array(items) => derive(items.iter().map(|item| evaluate(item, document)), None),
        Value::Object(map) => object(map, document),
        _ => Shape::constant(),
    }
}

pub fn derive(shapes: impl IntoIterator<Item = Shape>, operator: Option<&str>) -> Shape {
    let mut derivation = Derivation::default();
    for shape in shapes {
        shape.contribute(&mut derivation);
    }
    if let Some(name) = operator {
        derivation.operator(name);
    }
    Shape::Value(derivation.finish())
}

pub fn field_path(text: &str) -> Option<Vec<String>> {
    let segments: Vec<String> = text.split('.').map(str::to_string).collect();
    segments
        .iter()
        .all(|segment| !segment.is_empty() && !segment.starts_with('$'))
        .then_some(segments)
}

pub fn is_document_literal(map: &Map<String, Value>) -> bool {
    !map.is_empty() && !map.keys().any(|key| key.starts_with('$'))
}

fn variable(name: &str) -> Shape {
    let base = name.split('.').next().unwrap_or_default();
    if CONSTANT_VARIABLES.contains(&base) {
        Shape::constant()
    } else {
        Shape::unknown()
    }
}

fn object(map: &Map<String, Value>, document: &Shape) -> Shape {
    if map.is_empty()
        || (map.len() == 1
            && map
                .keys()
                .all(|key| EXTENDED_JSON_TYPES.contains(&key.as_str())))
    {
        return Shape::constant();
    }
    if is_document_literal(map) {
        let mut built = Shape::built();
        for (key, item) in map {
            if key.contains('.') {
                return Shape::unknown();
            }
            built.set(std::slice::from_ref(key), evaluate(item, document));
        }
        return built;
    }
    let mut entries = map.iter();
    match (entries.next(), entries.next()) {
        (Some((name, argument)), None) => operator(name, argument, document),
        _ => Shape::unknown(),
    }
}

fn operator(name: &str, argument: &Value, document: &Shape) -> Shape {
    if name == "$literal" {
        Shape::constant()
    } else if FIELD_NAME_OPERATORS.contains(&name) {
        Shape::unknown()
    } else {
        derive([evaluate(argument, document)], Some(name))
    }
}
