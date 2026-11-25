use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::{Cell, Clearance, Lineage};
use serde_json::Value;

use crate::anonymization_steps::support::read_column;
use crate::anonymization_world::{AnonymizationWorld, kind_named};

#[given(regex = r"^la colonne calculée lit une colonne personnelle$")]
async fn derived_from_personal(world: &mut AnonymizationWorld) {
    world.lineage = Some(vec![Lineage::Personal]);
}

#[given(regex = r"^la colonne calculée ne lit que des colonnes en clair$")]
async fn derived_from_clean(world: &mut AnonymizationWorld) {
    world.lineage = Some(vec![Lineage::Clean]);
}

#[given(regex = r"^la colonne calculée est prouvée en clair$")]
async fn derived_proven_clear(world: &mut AnonymizationWorld) {
    world.column_clearances = vec![Clearance::ProvenClear];
}

#[given(regex = r"^la politique déclare la colonne en clair$")]
async fn policy_declares_clear(world: &mut AnonymizationWorld) {
    world.column_clearances = vec![Clearance::ClearByPolicy];
}

#[given(regex = r#"^la politique déclare la colonne personnelle comme "([^"]+)"$"#)]
async fn policy_declares_personal(world: &mut AnonymizationWorld, kind: String) {
    world.column_clearances = vec![Clearance::PersonalByPolicy(kind_named(&kind))];
}

#[when(regex = r#"^l'agent lit la valeur "([^"]*)" dans la colonne calculée "([^"]+)"$"#)]
async fn agent_reads_derived_column(world: &mut AnonymizationWorld, value: String, column: String) {
    let clear = Value::String(value.clone());
    read_column(world, &column, false, "text", Cell::Text(value), clear);
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

#[then(regex = r#"^la valeur rendue est un jeton opaque "([^"]+)"$"#)]
async fn rendered_value_is_token(world: &mut AnonymizationWorld, label: String) {
    let rendered = world.last_pseudonym();
    let id = rendered
        .strip_prefix(&format!("<{label} id="))
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or_else(|| panic!("{rendered} is not a {label} token"));
    assert_eq!(id.len(), 8, "{rendered}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{rendered}");
}
