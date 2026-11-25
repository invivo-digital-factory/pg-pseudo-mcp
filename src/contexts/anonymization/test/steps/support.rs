use scarif_database_mcp::contexts::anonymization::domain::types::{Cell, ColumnMeta, ColumnSource};
use serde_json::{Value, json};

use crate::anonymization_world::AnonymizationWorld;

pub fn read_column(
    world: &mut AnonymizationWorld,
    column: &str,
    stored: bool,
    type_name: &str,
    cell: Cell,
    clear: Value,
) {
    let columns = [ColumnMeta {
        name: column.to_string(),
        type_name: type_name.to_string(),
        source: stored.then(|| ColumnSource {
            schema: "public".to_string(),
            table: "customers".to_string(),
            column: column.to_string(),
        }),
    }];
    let rows = world.use_cases().anonymize_result_set.execute(
        &columns,
        vec![vec![cell]],
        world.lineage.as_deref(),
        &world.injected,
        &world.enum_labels,
        &world.column_clearances,
    );
    let rendered = match &rows.rows[0][0] {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    world.input = Some(match &clear {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    });
    world.pseudonyms.push(rendered);
    world.clear_value = Some(clear);
    world.rows = Some(rows);
}

pub fn rendered_cell(world: &AnonymizationWorld) -> &Value {
    &world.rows.as_ref().expect("a value was read").rows[0][0]
}

pub fn typed_cell(type_name: &str, value: &str) -> (Cell, Value) {
    let text = || Value::String(value.to_string());
    match type_name {
        "int2" | "int4" | "int8" => {
            let number: i64 = value.parse().expect("an integer");
            (Cell::Int(number), json!(number))
        }
        "float8" => {
            let number: f64 = value.parse().expect("a float");
            (Cell::Float(number), json!(number))
        }
        "numeric" => (Cell::Numeric(value.to_string()), text()),
        "bool" => {
            let flag: bool = value.parse().expect("a boolean");
            (Cell::Bool(flag), json!(flag))
        }
        "uuid" => (Cell::Uuid(value.to_string()), text()),
        "date" => (Cell::Date(value.to_string()), text()),
        "timestamp" => (Cell::Timestamp(value.to_string()), text()),
        "timestamptz" => (Cell::TimestampTz(value.to_string()), text()),
        "inet" => (Cell::Inet(value.to_string()), text()),
        _ => (Cell::Text(value.to_string()), text()),
    }
}

pub const RESERVED_DOMAINS: [&str; 3] = ["example.com", "example.net", "example.org"];
