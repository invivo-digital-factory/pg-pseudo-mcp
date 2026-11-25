use cucumber::{then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::{ListTablesArgs, SearchSchemaArgs};
use serde_json::{Value, json};

use crate::bdd_support::CONNECTION;
use crate::querying_world::QueryingWorld;

fn checked(world: &QueryingWorld) -> Value {
    world.answer().checked_body()
}

fn quoted(text: &str) -> Vec<String> {
    text.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn table_names(world: &QueryingWorld) -> Vec<String> {
    checked(world)["tables"]
        .as_array()
        .expect("tables")
        .iter()
        .map(|table| table["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

async fn list_tables(world: &mut QueryingWorld, args: ListTablesArgs) {
    let result = tools::list_tables(
        world.contexts(),
        ListTablesArgs {
            connection: CONNECTION.into(),
            schema: Some("public".into()),
            ..args
        },
    )
    .await;
    world.record(&result);
}

#[when(regex = r"^l'agent liste les tables$")]
async fn agent_lists_tables(world: &mut QueryingWorld) {
    list_tables(world, ListTablesArgs::default()).await;
}

#[when(regex = r#"^l'agent liste les tables dont le nom contient "([^"]+)"$"#)]
async fn agent_lists_tables_like(world: &mut QueryingWorld, pattern: String) {
    list_tables(
        world,
        ListTablesArgs {
            name_like: Some(pattern),
            ..ListTablesArgs::default()
        },
    )
    .await;
}

#[when(regex = r"^l'agent liste (\d+) tables$")]
async fn agent_lists_a_page(world: &mut QueryingWorld, limit: u32) {
    list_tables(
        world,
        ListTablesArgs {
            limit: Some(limit),
            ..ListTablesArgs::default()
        },
    )
    .await;
}

#[then(regex = r"^la liste des tables contient (.+)$")]
async fn tables_contain(world: &mut QueryingWorld, names: String) {
    let listed = table_names(world);
    for name in quoted(&names) {
        assert!(listed.contains(&name), "{name} not in {listed:?}");
    }
}

#[then(regex = r"^la liste des tables est triée par nom$")]
async fn tables_are_sorted(world: &mut QueryingWorld) {
    let listed = table_names(world);
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted);
}

#[then(regex = r#"^la table "([^"]+)" compte (\d+) colonnes$"#)]
async fn table_column_count(world: &mut QueryingWorld, name: String, count: u64) {
    let body = checked(world);
    let table = body["tables"]
        .as_array()
        .expect("tables")
        .iter()
        .find(|table| table["name"] == name.as_str())
        .cloned()
        .unwrap_or_else(|| panic!("no table {name} in {body}"));
    assert_eq!(table["columns"], count, "{table}");
}

#[then(regex = r#"^la liste des tables vaut "([^"]+)"$"#)]
async fn tables_equal(world: &mut QueryingWorld, names: String) {
    let expected: Vec<String> = names.split(", ").map(str::to_string).collect();
    assert_eq!(table_names(world), expected);
}

#[then(regex = r"^la page suivante commence au décalage (\d+)$")]
async fn next_offset(world: &mut QueryingWorld, offset: u64) {
    let body = checked(world);
    assert_eq!(body["next_offset"], offset, "{body}");
}

#[then(regex = r#"^la description donne la clé primaire "([^"]+)"$"#)]
async fn primary_key(world: &mut QueryingWorld, column: String) {
    let body = checked(world);
    assert_eq!(body["primary_key"], json!([column]), "{body}");
}

#[then(regex = r#"^la description donne la clé étrangère "([^"]+)" vers "([^"]+)"$"#)]
async fn foreign_key(world: &mut QueryingWorld, column: String, target: String) {
    let body = checked(world);
    let found = body["foreign_keys"]
        .as_array()
        .expect("foreign keys")
        .iter()
        .any(|key| key["references"] == target.as_str() && key["columns"] == json!([column]));
    assert!(found, "{body}");
}

#[then(regex = r#"^la description donne l'index "([^"]+)" sur "([^"]+)"$"#)]
async fn index(world: &mut QueryingWorld, name: String, column: String) {
    let body = checked(world);
    let found = body["indexes"]
        .as_array()
        .expect("indexes")
        .iter()
        .any(|index| index["name"] == name.as_str() && index["columns"] == json!([column]));
    assert!(found, "{body}");
}

#[then(regex = r#"^la table est référencée par "([^"]+)" sur "([^"]+)"$"#)]
async fn referenced_by(world: &mut QueryingWorld, table: String, column: String) {
    let body = checked(world);
    let found = body["referenced_by"]
        .as_array()
        .expect("referenced_by")
        .iter()
        .any(|item| item["table"] == table.as_str() && item["columns"] == json!([column]));
    assert!(found, "{body}");
}

fn described_column(world: &QueryingWorld, name: &str) -> Value {
    let body = checked(world);
    body["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .find(|column| column["name"] == name)
        .cloned()
        .unwrap_or_else(|| panic!("no column {name} in {body}"))
}

#[then(regex = r#"^la colonne décrite "([^"]+)" sera pseudonymisée comme "([^"]+)"$"#)]
async fn described_column_kind(world: &mut QueryingWorld, name: String, kind: String) {
    assert_eq!(
        described_column(world, &name)["pseudonymized"],
        kind.as_str()
    );
}

#[then(regex = r#"^la colonne décrite "([^"]+)" ne sera pas pseudonymisée$"#)]
async fn described_column_clear(world: &mut QueryingWorld, name: String) {
    let column = described_column(world, &name);
    assert!(column.get("pseudonymized").is_none(), "{column}");
}

#[then(regex = r#"^le commentaire de la colonne "([^"]+)" commence par "([^"]+)"$"#)]
async fn column_comment_starts(world: &mut QueryingWorld, name: String, start: String) {
    let column = described_column(world, &name);
    let comment = column["comment"].as_str().expect("a comment");
    assert!(comment.starts_with(&start), "{comment}");
}

#[when(regex = r#"^l'agent cherche "([^"]*)" dans le schéma$"#)]
async fn agent_searches_schema(world: &mut QueryingWorld, pattern: String) {
    let result = tools::search_schema(
        world.contexts(),
        SearchSchemaArgs {
            connection: CONNECTION.into(),
            pattern,
            database: None,
        },
    )
    .await;
    world.record(&result);
}

fn matches(world: &QueryingWorld) -> Vec<Value> {
    checked(world)["matches"]
        .as_array()
        .expect("matches")
        .clone()
}

#[then(regex = r#"^la recherche trouve la colonne "([^"]+)" de "([^".]+)\.([^"]+)"$"#)]
async fn search_finds_column(
    world: &mut QueryingWorld,
    column: String,
    schema: String,
    table: String,
) {
    let found = matches(world).iter().any(|item| {
        item["schema"] == schema.as_str()
            && item["table"] == table.as_str()
            && item["column"] == column.as_str()
    });
    assert!(found, "{}", world.answer().text);
}

#[then(regex = r#"^la recherche trouve la table "([^".]+)\.([^"]+)"$"#)]
async fn search_finds_table(world: &mut QueryingWorld, schema: String, table: String) {
    let found = matches(world).iter().any(|item| {
        item["schema"] == schema.as_str()
            && item["table"] == table.as_str()
            && item.get("column").is_none()
    });
    assert!(found, "{}", world.answer().text);
}

#[then(regex = r"^la recherche ne trouve rien$")]
async fn search_finds_nothing(world: &mut QueryingWorld) {
    assert!(matches(world).is_empty(), "{}", world.answer().text);
}
