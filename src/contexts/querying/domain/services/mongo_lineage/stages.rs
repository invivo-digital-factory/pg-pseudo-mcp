use serde_json::{Map, Value};

use super::expressions::{
    derive, evaluate, field_path, is_document_literal, is_extended_json, single_entry,
};
use super::shape::Shape;
use crate::contexts::querying::domain::constants::clear_lineage::PASS_THROUGH_ACCUMULATORS;
use crate::contexts::querying::domain::constants::mongo_policy::WINDOW_ARGUMENT;

pub struct Pipeline<'a> {
    pub database: &'a str,
}

impl Pipeline<'_> {
    pub fn run(&self, start: Shape, stages: &[Value]) -> Shape {
        stages
            .iter()
            .fold(start, |document, stage| self.stage(document, stage))
    }

    fn stage(&self, document: Shape, stage: &Value) -> Shape {
        let Some((name, argument)) = stage_entry(stage) else {
            return Shape::unknown();
        };
        match name.as_str() {
            "$match" | "$sort" | "$skip" | "$limit" | "$sample" => document,
            "$project" => project(&document, argument),
            "$addFields" | "$set" => add_fields(document, argument),
            "$unset" => unset(document, argument),
            "$group" => group(&document, argument),
            "$count" => count(argument),
            "$sortByCount" => sort_by_count(&document, argument),
            "$unwind" => unwind(document, argument),
            "$lookup" => self.lookup(document, argument),
            "$graphLookup" => self.graph_lookup(document, argument),
            "$replaceRoot" => replace_root(&document, argument.get("newRoot")),
            "$replaceWith" => replace_root(&document, Some(argument)),
            "$bucket" => bucket(&document, argument),
            "$bucketAuto" => bucket_auto(&document, argument),
            "$setWindowFields" => window_fields(document, argument),
            "$fill" => forget_outputs(document, argument.get("output")),
            "$densify" => forget_fields(document, &[argument.get("field")]),
            "$geoNear" => forget_fields(
                document,
                &[argument.get("distanceField"), argument.get("includeLocs")],
            ),
            _ => Shape::unknown(),
        }
    }

    fn lookup(&self, mut document: Shape, argument: &Value) -> Shape {
        let Some(path) = named_path(argument.get("as")) else {
            return Shape::unknown();
        };
        let start = match argument.get("from") {
            Some(Value::String(collection)) => Shape::collection(self.database, collection),
            _ => Shape::unknown(),
        };
        let joined = match argument.get("pipeline") {
            None => start,
            Some(Value::Array(stages)) => self.run(start, stages),
            Some(_) => Shape::unknown(),
        };
        document.set(&path, joined);
        document
    }

    fn graph_lookup(&self, mut document: Shape, argument: &Value) -> Shape {
        let Some(path) = named_path(argument.get("as")) else {
            return Shape::unknown();
        };
        let mut found = match argument.get("from") {
            Some(Value::String(collection)) => Shape::collection(self.database, collection),
            _ => Shape::unknown(),
        };
        match argument
            .get("depthField")
            .map(|depth| named_path(Some(depth)))
        {
            None => {}
            Some(Some(depth)) => found.set(&depth, Shape::constant()),
            Some(None) => found = Shape::unknown(),
        }
        document.set(&path, found);
        document
    }
}

pub fn project(document: &Shape, argument: &Value) -> Shape {
    let Value::Object(spec) = argument else {
        return Shape::unknown();
    };
    if holds_extended_json(spec) {
        return Shape::unknown();
    }
    if is_exclusion(spec) {
        let mut projected = document.clone();
        exclude(&mut projected, &[], spec);
        projected
    } else {
        let mut projected = Shape::built();
        if !spec.contains_key("_id") {
            let id = ["_id".to_string()];
            projected.set(&id, document.at(&id));
        }
        include(&mut projected, document, &[], spec);
        projected
    }
}

fn include(projected: &mut Shape, document: &Shape, prefix: &[String], spec: &Map<String, Value>) {
    for (key, value) in spec {
        let Some(path) = spec_path(prefix, key) else {
            *projected = Shape::unknown();
            return;
        };
        match value {
            Value::Bool(_) | Value::Number(_) => {
                if is_truthy(value) {
                    projected.set(&path, document.at(&path));
                }
            }
            Value::Object(inner) if is_document_literal(inner) => {
                include(projected, document, &path, inner);
            }
            _ => projected.set(&path, evaluate(value, document)),
        }
    }
}

fn exclude(projected: &mut Shape, prefix: &[String], spec: &Map<String, Value>) {
    for (key, value) in spec {
        let Some(path) = spec_path(prefix, key) else {
            *projected = Shape::unknown();
            return;
        };
        match value {
            Value::Object(inner) => exclude(projected, &path, inner),
            _ if !is_truthy(value) => projected.set(&path, Shape::unknown()),
            _ => {}
        }
    }
}

fn holds_extended_json(spec: &Map<String, Value>) -> bool {
    spec.values().any(|value| match value {
        Value::Object(inner) if is_document_literal(inner) => holds_extended_json(inner),
        Value::Object(inner) => is_extended_json(inner),
        _ => false,
    })
}

fn is_exclusion(spec: &Map<String, Value>) -> bool {
    let mut others = spec
        .iter()
        .filter(|(key, _)| key.as_str() != "_id")
        .map(|(_, value)| value)
        .peekable();
    if others.peek().is_none() {
        spec.get("_id").is_some_and(is_all_falsy)
    } else {
        others.all(is_all_falsy)
    }
}

fn is_all_falsy(value: &Value) -> bool {
    match value {
        Value::Bool(_) | Value::Number(_) => !is_truthy(value),
        Value::Object(inner) if is_document_literal(inner) => inner.values().all(is_all_falsy),
        _ => false,
    }
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|number| number != 0.0),
        _ => true,
    }
}

fn add_fields(document: Shape, argument: &Value) -> Shape {
    let Value::Object(spec) = argument else {
        return Shape::unknown();
    };
    let mut added = document.clone();
    if set_fields(&mut added, &document, &[], spec) {
        added
    } else {
        Shape::unknown()
    }
}

fn set_fields(
    added: &mut Shape,
    document: &Shape,
    prefix: &[String],
    spec: &Map<String, Value>,
) -> bool {
    spec.iter().all(|(key, value)| {
        let Some(path) = spec_path(prefix, key) else {
            return false;
        };
        match value {
            Value::Object(inner) if is_document_literal(inner) => {
                set_fields(added, document, &path, inner)
            }
            _ => {
                added.set(&path, evaluate(value, document));
                true
            }
        }
    })
}

fn unset(mut document: Shape, argument: &Value) -> Shape {
    let names: Vec<&Value> = match argument {
        Value::Array(items) => items.iter().collect(),
        single => vec![single],
    };
    for name in names {
        let Some(path) = named_path(Some(name)) else {
            return Shape::unknown();
        };
        document.set(&path, Shape::unknown());
    }
    document
}

fn group(document: &Shape, argument: &Value) -> Shape {
    let Value::Object(spec) = argument else {
        return Shape::unknown();
    };
    let mut grouped = Shape::built();
    for (key, value) in spec {
        let Some(path) = spec_path(&[], key) else {
            return Shape::unknown();
        };
        let output = if key == "_id" {
            evaluate(value, document)
        } else {
            accumulate(value, document, false)
        };
        grouped.set(&path, output);
    }
    grouped
}

fn accumulate(value: &Value, document: &Shape, in_window: bool) -> Shape {
    let Value::Object(map) = value else {
        return Shape::unknown();
    };
    let mut entries = map
        .iter()
        .filter(|(key, _)| !(in_window && key.as_str() == WINDOW_ARGUMENT));
    let (Some((name, argument)), None) = (entries.next(), entries.next()) else {
        return Shape::unknown();
    };
    if PASS_THROUGH_ACCUMULATORS.contains(&name.as_str()) {
        evaluate(argument, document)
    } else if name.starts_with('$') {
        derive([evaluate(argument, document)], Some(name))
    } else {
        Shape::unknown()
    }
}

fn accumulate_outputs(
    target: &mut Shape,
    document: &Shape,
    outputs: &Map<String, Value>,
    in_window: bool,
) -> bool {
    outputs
        .iter()
        .all(|(key, value)| match spec_path(&[], key) {
            Some(path) => {
                target.set(&path, accumulate(value, document, in_window));
                true
            }
            None => false,
        })
}

fn count(argument: &Value) -> Shape {
    let Some(path) = named_path(Some(argument)) else {
        return Shape::unknown();
    };
    let mut counted = Shape::built();
    counted.set(&path, Shape::counted(&["$count"]));
    counted
}

fn sort_by_count(document: &Shape, argument: &Value) -> Shape {
    let mut counted = Shape::built();
    counted.set(&["_id".to_string()], evaluate(argument, document));
    counted.set(&["count".to_string()], Shape::counted(&["$sum"]));
    counted
}

fn unwind(mut document: Shape, argument: &Value) -> Shape {
    match argument.get("includeArrayIndex") {
        None => document,
        Some(index) => match named_path(Some(index)) {
            Some(path) => {
                document.set(&path, Shape::constant());
                document
            }
            None => Shape::unknown(),
        },
    }
}

fn replace_root(document: &Shape, new_root: Option<&Value>) -> Shape {
    match new_root {
        Some(Value::String(text)) if text.starts_with('$') && !text.starts_with("$$") => {
            evaluate(&Value::String(text.clone()), document)
        }
        Some(value @ Value::Object(map)) if is_document_literal(map) => evaluate(value, document),
        _ => Shape::unknown(),
    }
}

fn bucket(document: &Shape, argument: &Value) -> Shape {
    let Some(group_by) = argument.get("groupBy") else {
        return Shape::unknown();
    };
    let mut bucketed = Shape::built();
    bucketed.set(
        &["_id".to_string()],
        derive([evaluate(group_by, document)], None),
    );
    with_bucket_outputs(bucketed, document, argument)
}

fn bucket_auto(document: &Shape, argument: &Value) -> Shape {
    let Some(group_by) = argument.get("groupBy") else {
        return Shape::unknown();
    };
    let mut bucketed = Shape::built();
    for bound in ["min", "max"] {
        bucketed.set(
            &["_id".to_string(), bound.to_string()],
            derive([evaluate(group_by, document)], Some(&format!("${bound}"))),
        );
    }
    with_bucket_outputs(bucketed, document, argument)
}

fn with_bucket_outputs(mut bucketed: Shape, document: &Shape, argument: &Value) -> Shape {
    match argument.get("output") {
        None => {
            bucketed.set(&["count".to_string()], Shape::counted(&["$sum"]));
            bucketed
        }
        Some(Value::Object(outputs))
            if accumulate_outputs(&mut bucketed, document, outputs, false) =>
        {
            bucketed
        }
        Some(_) => Shape::unknown(),
    }
}

fn window_fields(document: Shape, argument: &Value) -> Shape {
    let mut windowed = document.clone();
    match argument.get("output") {
        Some(Value::Object(outputs))
            if accumulate_outputs(&mut windowed, &document, outputs, true) =>
        {
            windowed
        }
        _ => Shape::unknown(),
    }
}

fn forget_outputs(document: Shape, outputs: Option<&Value>) -> Shape {
    match outputs {
        Some(Value::Object(outputs)) => forget(document, outputs.keys().map(|key| field_path(key))),
        _ => Shape::unknown(),
    }
}

fn forget_fields(document: Shape, names: &[Option<&Value>]) -> Shape {
    forget(
        document,
        names.iter().flatten().map(|name| named_path(Some(name))),
    )
}

fn forget(mut document: Shape, paths: impl Iterator<Item = Option<Vec<String>>>) -> Shape {
    for path in paths {
        let Some(path) = path else {
            return Shape::unknown();
        };
        document.set(&path, Shape::unknown());
    }
    document
}

fn stage_entry(stage: &Value) -> Option<(&String, &Value)> {
    match stage {
        Value::Object(map) => single_entry(map),
        _ => None,
    }
}

fn named_path(name: Option<&Value>) -> Option<Vec<String>> {
    match name {
        Some(Value::String(text)) => field_path(text),
        _ => None,
    }
}

fn spec_path(prefix: &[String], key: &str) -> Option<Vec<String>> {
    let key = key.strip_suffix(".$").unwrap_or(key);
    let mut path = prefix.to_vec();
    path.extend(field_path(key)?);
    Some(path)
}
