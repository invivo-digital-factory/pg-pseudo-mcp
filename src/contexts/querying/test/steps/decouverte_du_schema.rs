use cucumber::{then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::{
    DescribeTableArgs, ListTablesArgs, SearchSchemaArgs,
};
use serde_json::{Value, json};

use crate::bdd_support::CONNECTION;
use crate::querying_steps::support::{checked, list_tables};
use crate::querying_world::{MONGO_CONNECTION, QueryingWorld};

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

#[when(regex = r"^l'agent liste les collections$")]
async fn list_collections(world: &mut QueryingWorld) {
    let result = tools::list_tables(
        world.contexts(),
        ListTablesArgs {
            connection: MONGO_CONNECTION.into(),
            ..ListTablesArgs::default()
        },
    )
    .await;
    world.record(&result);
}

#[when(regex = r#"^l'agent décrit la collection "([^"]+)"$"#)]
async fn describe_collection(world: &mut QueryingWorld, collection: String) {
    let result = tools::describe_table(
        world.contexts(),
        DescribeTableArgs {
            connection: MONGO_CONNECTION.into(),
            table: collection,
            database: None,
        },
    )
    .await;
    world.record(&result);
}

#[when(regex = r#"^l'agent cherche "([^"]+)" dans les collections$"#)]
async fn search_collections(world: &mut QueryingWorld, pattern: String) {
    let result = tools::search_schema(
        world.contexts(),
        SearchSchemaArgs {
            connection: MONGO_CONNECTION.into(),
            pattern,
            database: None,
        },
    )
    .await;
    world.record(&result);
}

fn collections(world: &QueryingWorld) -> Vec<(String, String)> {
    checked(world)["collections"]
        .as_array()
        .expect("collections")
        .iter()
        .map(|entry| {
            (
                entry["name"].as_str().unwrap_or_default().to_string(),
                entry["kind"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

#[then(regex = r#"^la liste contient la collection "([^"]+)" de type "([^"]+)"$"#)]
async fn list_contains_collection(world: &mut QueryingWorld, name: String, kind: String) {
    let listed = collections(world);
    assert!(listed.contains(&(name, kind)), "{listed:?}");
}

#[then(regex = r"^la liste ne contient aucune collection système$")]
async fn list_has_no_system_collection(world: &mut QueryingWorld) {
    let listed = collections(world);
    assert!(!listed.is_empty());
    assert!(
        listed.iter().all(|(name, _)| !name.starts_with("system.")),
        "{listed:?}"
    );
}

fn described_field(world: &QueryingWorld, path: &str) -> Value {
    let body = checked(world);
    body["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .find(|field| field["path"] == path)
        .cloned()
        .unwrap_or_else(|| panic!("no field {path} in {body}"))
}

#[then(regex = r#"^le champ décrit "([^"]+)" est de type "([^"]+)"$"#)]
async fn described_field_type(world: &mut QueryingWorld, path: String, kind: String) {
    assert_eq!(described_field(world, &path)["types"], json!([kind]));
}

#[then(regex = r#"^le champ décrit "([^"]+)" sera pseudonymisé comme "([^"]+)"$"#)]
async fn described_field_pseudonymized(world: &mut QueryingWorld, path: String, kind: String) {
    assert_eq!(
        described_field(world, &path)["pseudonymized"],
        kind.as_str()
    );
}

#[then(
    regex = r#"^le champ décrit "([^"]+)" est de type "([^"]+)" et sera pseudonymisé comme "([^"]+)"$"#
)]
async fn described_field_type_and_kind(
    world: &mut QueryingWorld,
    path: String,
    kind: String,
    pseudonym: String,
) {
    let field = described_field(world, &path);
    assert_eq!(field["types"], json!([kind]), "{field}");
    assert_eq!(field["pseudonymized"], pseudonym.as_str(), "{field}");
}

#[then(regex = r#"^la description donne un index sur "([^"]+)"$"#)]
async fn described_index(world: &mut QueryingWorld, path: String) {
    let body = checked(world);
    assert!(body["indexes"].to_string().contains(&path), "{body}");
}

#[then(regex = r#"^la recherche trouve le champ "([^"]+)"$"#)]
async fn search_finds_field(world: &mut QueryingWorld, path: String) {
    let body = checked(world);
    let found = body["matches"]
        .as_array()
        .expect("matches")
        .iter()
        .any(|item| item["column"] == path.as_str());
    assert!(found, "{body}");
}
