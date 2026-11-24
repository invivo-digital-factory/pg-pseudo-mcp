use serde_json::Value;

const WRITE_STAGES: [&str; 2] = ["$out", "$merge"];

pub fn pipeline_writes(pipeline: &[Value]) -> bool {
    pipeline.iter().any(writes)
}

fn writes(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields
            .iter()
            .any(|(key, nested)| WRITE_STAGES.contains(&key.as_str()) || writes(nested)),
        Value::Array(items) => items.iter().any(writes),
        _ => false,
    }
}
