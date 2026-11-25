use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::{Cell, ColumnMeta, ColumnSource};
use serde_json::Value;

use crate::anonymization_world::{AnonymizationWorld, listed};

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

fn rendered_cell(world: &AnonymizationWorld) -> &Value {
    &world.rows.as_ref().expect("a value was read").rows[0][0]
}

#[given(regex = r#"^la base déclare les libellés d'énumération "([^"]+)"$"#)]
async fn database_declares_enum_labels(world: &mut AnonymizationWorld, labels: String) {
    world.enum_labels = listed(&labels);
}

#[when(regex = r#"^l'agent lit la valeur "([^"]*)" dans la colonne "([^"]+)"$"#)]
async fn agent_reads_text_column(world: &mut AnonymizationWorld, value: String, column: String) {
    let clear = Value::String(value.clone());
    read_column(world, &column, true, "text", Cell::Text(value), clear);
}

#[then(regex = r"^la valeur reste en clair$")]
async fn value_stays_clear(world: &mut AnonymizationWorld) {
    let clear = world.clear_value.as_ref().expect("a value was read");
    assert_eq!(rendered_cell(world), clear);
}

#[then(regex = r"^la valeur est pseudonymisée$")]
async fn value_is_pseudonymized(world: &mut AnonymizationWorld) {
    let clear = world.clear_value.as_ref().expect("a value was read");
    assert_ne!(rendered_cell(world), clear);
}

#[then(regex = r#"^la colonne est signalée "([^"]+)" pour la raison "([^"]+)"$"#)]
async fn column_is_flagged(world: &mut AnonymizationWorld, kind: String, why: String) {
    let rows = world.rows.as_ref().expect("a value was read");
    let flag = rows.per_column[0]
        .as_ref()
        .unwrap_or_else(|| panic!("the column is not flagged: {:?}", rows.rows));
    assert_eq!(flag.kind, kind);
    assert_eq!(
        serde_json::to_value(flag.why).expect("a reason"),
        Value::String(why)
    );
}

#[then(regex = r#"^la colonne est signalée pour la raison "([^"]+)"$"#)]
async fn column_is_flagged_for(world: &mut AnonymizationWorld, why: String) {
    let rows = world.rows.as_ref().expect("a value was read");
    let flag = rows.per_column[0].as_ref().expect("the column is flagged");
    assert_eq!(
        serde_json::to_value(flag.why).expect("a reason"),
        Value::String(why)
    );
}

#[then(regex = r"^la colonne n'est pas signalée$")]
async fn column_is_not_flagged(world: &mut AnonymizationWorld) {
    let rows = world.rows.as_ref().expect("a value was read");
    assert_eq!(rows.per_column[0], None, "{:?}", rows.rows);
    assert!(rows.pseudonymized.is_empty());
}

#[then(regex = r"^la valeur rendue est un nombre$")]
async fn rendered_value_is_a_number(world: &mut AnonymizationWorld) {
    let rendered = rendered_cell(world);
    assert!(rendered.is_number(), "{rendered}");
}
