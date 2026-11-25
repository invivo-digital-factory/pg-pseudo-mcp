use std::net::Ipv4Addr;

use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::Cell;
use serde_json::Value;

use crate::anonymization_steps::support::{
    RESERVED_DOMAINS, read_column, rendered_cell, typed_cell,
};
use crate::anonymization_world::{AnonymizationWorld, kind_named, layout};

#[then(regex = r#"^le pseudonyme a la présentation "([^"]+)"$"#)]
async fn pseudonym_has_layout(world: &mut AnonymizationWorld, expected: String) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(layout(pseudonym), expected, "{pseudonym}");
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

#[then(regex = r"^la colonne n'est pas signalée$")]
async fn column_is_not_flagged(world: &mut AnonymizationWorld) {
    let rows = world.rows.as_ref().expect("a value was read");
    assert_eq!(rows.per_column[0], None, "{:?}", rows.rows);
    assert!(rows.pseudonymized.is_empty());
}

#[when(regex = r#"^l'agent lit "([^"]*)" dans la colonne "([^"]+)" de type "([^"]+)"$"#)]
async fn agent_reads_typed_column(
    world: &mut AnonymizationWorld,
    value: String,
    column: String,
    type_name: String,
) {
    let (cell, clear) = typed_cell(&type_name, &value);
    read_column(world, &column, true, &type_name, cell, clear);
}

const DOCUMENTATION_IPV4_PREFIXES: [&str; 3] = ["192.0.2.", "198.51.100.", "203.0.113."];

#[then(regex = r"^le pseudonyme diffère de la valeur$")]
async fn pseudonym_differs(world: &mut AnonymizationWorld) {
    assert_ne!(world.last_pseudonym(), world.input());
}

#[then(regex = r"^le pseudonyme est une adresse sur example\.com, example\.net ou example\.org$")]
async fn pseudonym_is_reserved_email(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let (local, domain) = pseudonym.split_once('@').expect("an email");
    assert!(RESERVED_DOMAINS.contains(&domain), "{pseudonym}");
    assert!(
        local
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-'),
        "{pseudonym}"
    );
}

#[then(regex = r"^le pseudonyme est une adresse des plages IPv4 de documentation$")]
async fn pseudonym_is_documentation_ipv4(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert!(pseudonym.parse::<Ipv4Addr>().is_ok(), "{pseudonym}");
    assert!(
        DOCUMENTATION_IPV4_PREFIXES
            .iter()
            .any(|prefix| pseudonym.starts_with(prefix)),
        "{pseudonym}"
    );
}

#[then(regex = r"^le pseudonyme (\d+) diffère du pseudonyme (\d+)$")]
async fn nth_pseudonyms_differ(world: &mut AnonymizationWorld, left: usize, right: usize) {
    assert_ne!(world.nth_pseudonym(left), world.nth_pseudonym(right));
}

#[given(regex = r"^un répertoire de données neuf$")]
async fn fresh_data_directory(world: &mut AnonymizationWorld) {
    world.pseudonyms.clear();
    world.translation = None;
    assert!(
        std::fs::read_dir(world.data_dir.path())
            .expect("data dir")
            .next()
            .is_none()
    );
}

#[given(regex = r"^un second poste avec son propre répertoire de données$")]
async fn second_workstation(world: &mut AnonymizationWorld) {
    world.other_data_dir = Some(tempfile::tempdir().expect("second data dir"));
}

#[when(regex = r#"^l'agent obtient le pseudonyme de l'adresse "([^"]+)"$"#)]
async fn agent_gets_pseudonym(world: &mut AnonymizationWorld, email: String) {
    let pseudonym = world.use_cases().anonymize_free_text.execute(&email, &[]);
    world.pseudonyms = vec![pseudonym];
}

#[when(regex = r#"^l'agent obtient le pseudonyme de type "([^"]+)" de "([^"]*)"$"#)]
async fn agent_gets_pseudonym_of_kind(world: &mut AnonymizationWorld, kind: String, value: String) {
    let pseudonym = world
        .pseudonymizer()
        .pseudonymize(kind_named(&kind), &value)
        .text;
    world.input = Some(value);
    world.pseudonyms.push(pseudonym);
}

#[when(regex = r"^le développeur purge la table de correspondance$")]
async fn developer_purges_mapping(world: &mut AnonymizationWorld) {
    world.purge();
}

#[then(regex = r#"^la valeur rendue ne contient plus "([^"]+)"$"#)]
async fn rendered_value_lacks(world: &mut AnonymizationWorld, fragment: String) {
    let rendered = world.last_pseudonym();
    for form in [
        fragment.clone(),
        fragment.to_lowercase(),
        fragment.to_uppercase(),
    ] {
        assert!(!rendered.contains(&form), "{form} leaked in {rendered}");
    }
}

#[then(regex = r#"^la valeur rendue contient "([^"]+)"$"#)]
async fn rendered_value_contains(world: &mut AnonymizationWorld, fragment: String) {
    let fragment = world.expand(&fragment);
    let rendered = world.last_pseudonym();
    assert!(
        rendered.contains(&fragment),
        "{fragment} missing in {rendered}"
    );
}
