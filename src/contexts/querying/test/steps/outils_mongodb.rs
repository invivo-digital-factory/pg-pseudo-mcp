use cucumber::{given, then, when};
use scarif_database_mcp::composition::tools;
use scarif_database_mcp::contexts::querying::presentation::{
    AggregateArgs, DescribeTableArgs, FindArgs, ListDatabasesArgs, ListTablesArgs, QueryArgs,
    SearchSchemaArgs,
};
use serde_json::{Value, json};

use crate::querying_world::{MONGO_CONNECTION, MONGO_READER, MongoUser, QueryingWorld};

#[given(regex = r"^la connexion MongoDB de test à travers le bastion$")]
async fn mongo_connection(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb();
}

#[given(regex = r"^la connexion MongoDB de test avec un utilisateur autorisé à écrire$")]
async fn mongo_writer_connection(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb_as(&MongoUser {
        name: "atlas-writer",
        password: "atlas-writer-test-pw",
        default_database: true,
    });
}

#[given(regex = r"^la connexion MongoDB de test sans base par défaut$")]
async fn mongo_connection_without_database(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb_as(&MongoUser {
        default_database: false,
        ..MONGO_READER
    });
}

#[when(regex = r#"^l'agent exécute la requête SQL "(.+)" sur la connexion MongoDB$"#)]
async fn sql_on_mongo(world: &mut QueryingWorld, sql: String) {
    let result = tools::query(
        world.contexts(),
        QueryArgs {
            connection: MONGO_CONNECTION.into(),
            sql,
            ..QueryArgs::default()
        },
    )
    .await;
    world.record(&result);
}

async fn find(world: &mut QueryingWorld, args: FindArgs) {
    let result = tools::find(
        world.contexts(),
        FindArgs {
            connection: MONGO_CONNECTION.into(),
            ..args
        },
    )
    .await;
    world.record(&result);
}

async fn aggregate(world: &mut QueryingWorld, args: AggregateArgs) {
    world.pipeline = Some(args.pipeline.clone());
    let result = tools::aggregate(
        world.contexts(),
        AggregateArgs {
            connection: MONGO_CONNECTION.into(),
            ..args
        },
    )
    .await;
    world.record(&result);
}

fn by_id(collection: &str, id: &str) -> FindArgs {
    FindArgs {
        collection: collection.into(),
        filter: Some(json!({"_id": {"$oid": id}})),
        ..FindArgs::default()
    }
}

#[when(regex = r#"^l'agent cherche dans "([^"]+)" le document d'identifiant "([0-9a-f]{24})"$"#)]
async fn find_by_id(world: &mut QueryingWorld, collection: String, id: String) {
    find(world, by_id(&collection, &id)).await;
}

#[when(
    regex = r#"^l'agent cherche dans la base "([^"]+)" de la collection "([^"]+)" le document d'identifiant "([0-9a-f]{24})"$"#
)]
async fn find_by_id_in_database(
    world: &mut QueryingWorld,
    database: String,
    collection: String,
    id: String,
) {
    find(
        world,
        FindArgs {
            database: Some(database),
            ..by_id(&collection, &id)
        },
    )
    .await;
}

#[when(
    regex = r#"^l'agent cherche dans "([^"]+)" les documents dont "([^"]+)" vaut ce pseudonyme$"#
)]
async fn find_by_pseudonym(world: &mut QueryingWorld, collection: String, path: String) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was seen");
    find(
        world,
        FindArgs {
            collection,
            filter: Some(json!({ path: pseudonym })),
            ..FindArgs::default()
        },
    )
    .await;
}

#[when(regex = r#"^l'agent cherche dans "([^"]+)" tous les documents$"#)]
async fn find_all(world: &mut QueryingWorld, collection: String) {
    find(
        world,
        FindArgs {
            collection,
            ..FindArgs::default()
        },
    )
    .await;
}

#[when(regex = r#"^l'agent cherche dans "([^"]+)" tous les documents avec max_bytes à (\d+)$"#)]
async fn find_all_within_bytes(world: &mut QueryingWorld, collection: String, bytes: u64) {
    find(
        world,
        FindArgs {
            collection,
            max_bytes: Some(bytes),
            ..FindArgs::default()
        },
    )
    .await;
}

#[when(regex = r"^l'agent agrège (.+) avec (\[.*\])$")]
async fn aggregate_inline(world: &mut QueryingWorld, collection: String, pipeline: String) {
    let pipeline: Value = serde_json::from_str(&pipeline).expect("a JSON pipeline");
    aggregate(
        world,
        AggregateArgs {
            collection: collection.trim_matches('"').into(),
            pipeline: pipeline.as_array().expect("a pipeline").clone(),
            ..AggregateArgs::default()
        },
    )
    .await;
}

#[when(
    regex = r#"^l'agent lit dans "([^"]+)" un texte de (\d+) caractères et un tableau de (\d+) éléments$"#
)]
async fn aggregate_long_values(
    world: &mut QueryingWorld,
    collection: String,
    characters: usize,
    items: u64,
) {
    let text = "abcdefghij".repeat(characters / 10);
    let pipeline = vec![
        json!({"$match": {"_id": {"$oid": "65a000000000000000000001"}}}),
        json!({"$project": {"_id": 0, "note": {"$literal": text}, "items": {"$range": [0, items]}}}),
    ];
    aggregate(
        world,
        AggregateArgs {
            collection,
            pipeline,
            ..AggregateArgs::default()
        },
    )
    .await;
}

#[when(regex = r"^l'agent relit le même texte avec cell_offset à (\d+)$")]
async fn aggregate_again_from_offset(world: &mut QueryingWorld, offset: u64) {
    let pipeline = world.pipeline.clone().expect("an aggregation was run");
    aggregate(
        world,
        AggregateArgs {
            collection: "customers".into(),
            pipeline,
            cell_offset: Some(offset),
            ..AggregateArgs::default()
        },
    )
    .await;
}

#[when(regex = r"^l'agent liste les bases de la connexion MongoDB$")]
async fn list_mongo_databases(world: &mut QueryingWorld) {
    let result = tools::list_databases(
        world.contexts(),
        ListDatabasesArgs {
            connection: MONGO_CONNECTION.into(),
        },
    )
    .await;
    world.record(&result);
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

fn checked(world: &QueryingWorld) -> Value {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    answer.body()
}

fn first_document_field(world: &QueryingWorld, field: &str) -> Value {
    let body = checked(world);
    body["documents"][0][field].clone()
}

#[then(regex = r#"^le champ "([^"]+)" est pseudonymisé comme "([^"]+)"$"#)]
async fn flagged(world: &mut QueryingWorld, path: String, kind: String) {
    let body = checked(world);
    assert_eq!(body["pseudonymized"][&path]["kind"], kind, "{body}");
    let value = path
        .split('.')
        .fold(&body["documents"][0], |node, key| &node[key])
        .as_str()
        .expect("a text value")
        .to_string();
    world.pseudonym = Some(value);
}

#[then(regex = r"^la réponse compte (\d+) documents$")]
async fn document_count(world: &mut QueryingWorld, count: usize) {
    let body = checked(world);
    assert_eq!(body["document_count"], count, "{body}");
}

#[then(regex = r"^un pseudonyme a été retraduit$")]
async fn translated(world: &mut QueryingWorld) {
    let body = checked(world);
    assert_eq!(body["translated_literals"], 1, "{body}");
}

#[then(
    regex = r#"^le champ "([^"]+)" du premier document compte (\d+) caractères suivis de "([^"]+)"$"#
)]
async fn first_document_text_cut(
    world: &mut QueryingWorld,
    field: String,
    characters: usize,
    marker: String,
) {
    let value = first_document_field(world, &field);
    let text = value.as_str().expect("a text value");
    let kept = text
        .strip_suffix(marker.as_str())
        .unwrap_or_else(|| panic!("{text} does not end with {marker}"));
    assert_eq!(kept.chars().count(), characters, "{text}");
}

#[then(regex = r#"^le champ "([^"]+)" du premier document commence par "([^"]+)"$"#)]
async fn first_document_text_starts(world: &mut QueryingWorld, field: String, start: String) {
    let value = first_document_field(world, &field);
    let text = value.as_str().expect("a text value");
    assert!(text.starts_with(&start), "{text}");
}

#[then(regex = r#"^le champ "([^"]+)" du premier document compte (\d+) éléments$"#)]
async fn first_document_array_len(world: &mut QueryingWorld, field: String, count: usize) {
    let value = first_document_field(world, &field);
    assert_eq!(value.as_array().expect("an array").len(), count, "{value}");
}

#[then(regex = r#"^la base de la connexion est "([^"]+)"$"#)]
async fn default_database_is(world: &mut QueryingWorld, database: String) {
    let body = checked(world);
    assert_eq!(body["default"], database.as_str(), "{body}");
}

#[then(regex = r#"^la liste ne contient ni "([^"]+)", ni "([^"]+)", ni "([^"]+)"$"#)]
async fn list_excludes(world: &mut QueryingWorld, first: String, second: String, third: String) {
    let body = checked(world);
    let databases = body["databases"].as_array().expect("databases");
    for name in [first, second, third] {
        assert!(!databases.contains(&json!(name)), "{body}");
    }
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

#[then(regex = r#"^l'outil est refusé et renvoie vers "([^"]+)"$"#)]
async fn redirected(world: &mut QueryingWorld, tool: String) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    let body = answer.body();
    assert_eq!(body["error"]["code"], "wrong_engine");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains(&tool),
        "{body}"
    );
}

#[then(regex = r#"^l'outil est refusé avec le code "([^"]+)"$"#)]
async fn refused_with(world: &mut QueryingWorld, code: String) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    assert_eq!(
        answer.body()["error"]["code"],
        code.as_str(),
        "{}",
        answer.text
    );
}

#[then(regex = r#"^la réponse ne contient pas "([^"]+)"$"#)]
async fn absent(world: &mut QueryingWorld, text: String) {
    assert!(
        !world.answer().text.contains(&text),
        "{}",
        world.answer().text
    );
}

#[then(regex = r#"^la réponse contient "([^"]+)"$"#)]
async fn present(world: &mut QueryingWorld, text: String) {
    assert!(
        world.answer().text.contains(&text),
        "{}",
        world.answer().text
    );
}
