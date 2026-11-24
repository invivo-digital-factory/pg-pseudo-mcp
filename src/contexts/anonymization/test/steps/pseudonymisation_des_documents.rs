use std::collections::BTreeSet;

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::{
    AnonymizedDocuments, Cell, ColumnMeta, ColumnSource, FieldClearance, FieldPath, Kind, Node,
};
use serde_json::Value;

use crate::anonymization_world::AnonymizationWorld;

fn node(value: &Value) -> Node {
    match value {
        Value::Null => Node::Null,
        Value::Bool(flag) => Node::Bool(*flag),
        Value::Number(number) => number.as_i64().map_or_else(
            || Node::Double(number.as_f64().expect("a number")),
            Node::Int,
        ),
        Value::String(text) => Node::Text(text.clone()),
        Value::Array(items) => Node::Array(items.iter().map(node).collect()),
        Value::Object(entries) => match entries.iter().next() {
            Some((key, Value::String(inner))) if entries.len() == 1 && key == "$oid" => {
                Node::ObjectId(inner.clone())
            }
            Some((key, Value::String(inner))) if entries.len() == 1 && key == "$date" => {
                Node::Date(inner.clone())
            }
            Some((key, Value::String(inner))) if entries.len() == 1 && key == "$numberLong" => {
                Node::Int(inner.parse().expect("an Int64"))
            }
            _ => Node::Object(
                entries
                    .iter()
                    .map(|(key, item)| (key.clone(), node(item)))
                    .collect(),
            ),
        },
    }
}

fn field<'a>(document: &'a Node, path: &str) -> &'a Node {
    path.split('.')
        .fold(document, |current, key| match current {
            Node::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, child)| child)
                .unwrap_or_else(|| panic!("no field {key} in {current:?}")),
            other => panic!("{other:?} has no field {key}"),
        })
}

fn listed(values: &str) -> Vec<String> {
    values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn kind_named(name: &str) -> Kind {
    Kind::ALL
        .into_iter()
        .find(|kind| kind.as_str() == name)
        .unwrap_or_else(|| panic!("unknown kind {name}"))
}

fn anonymized(world: &AnonymizationWorld) -> &AnonymizedDocuments {
    world.anonymized.as_ref().expect("the document was read")
}

fn read_field<'a>(world: &'a AnonymizationWorld, path: &str) -> (&'a Node, &'a Node) {
    let input = world.document.as_ref().expect("a document");
    let output = &anonymized(world).documents[0];
    (field(input, path), field(output, path))
}

#[given(regex = r"^le document suivant :$")]
async fn given_document(world: &mut AnonymizationWorld, step: &Step) {
    let text = step.docstring.as_deref().expect("a docstring");
    let value: Value = serde_json::from_str(text).expect("a JSON document");
    world.document = Some(node(&value));
}

#[given(regex = r#"^un document de (\d+) champs inconnus valant "([^"]+)"$"#)]
async fn given_wide_document(world: &mut AnonymizationWorld, count: usize, value: String) {
    world.document = Some(Node::Object(
        (0..count)
            .map(|index| (format!("remarque{index}"), Node::Text(value.clone())))
            .collect(),
    ));
}

#[given(regex = r#"^la politique déclare le champ "([^"]+)" personnel comme "([^"]+)"$"#)]
async fn given_personal_field(world: &mut AnonymizationWorld, path: String, kind: String) {
    world.clearances.push((
        FieldPath::new(&path),
        FieldClearance::Personal(kind_named(&kind)),
    ));
}

#[given(regex = r#"^la politique déclare le champ "([^"]+)" en clair$"#)]
async fn given_clear_field(world: &mut AnonymizationWorld, path: String) {
    world
        .clearances
        .push((FieldPath::new(&path), FieldClearance::Clear));
}

#[when(regex = r"^l'agent lit ce document$")]
async fn agent_reads_document(world: &mut AnonymizationWorld) {
    let document = world.document.clone().expect("a document");
    let anonymized =
        world
            .use_cases()
            .anonymize_documents
            .execute(vec![document], &world.clearances, &[]);
    world.anonymized = Some(anonymized);
}

#[when(regex = r#"^l'agent lit une ligne PostgreSQL dont la colonne "([^"]+)" vaut "([^"]+)"$"#)]
async fn agent_reads_row(world: &mut AnonymizationWorld, column: String, value: String) {
    let columns = [ColumnMeta {
        name: column.clone(),
        type_name: "text".to_string(),
        source: Some(ColumnSource {
            schema: "public".to_string(),
            table: "customers".to_string(),
            column,
        }),
    }];
    let anonymized = world.use_cases().anonymize_result_set.execute(
        &columns,
        vec![vec![Cell::Text(value.clone())]],
        None,
        &[],
        &[],
        &[],
    );
    let pseudonym = anonymized.rows[0][0].as_str().expect("text").to_string();
    assert_ne!(pseudonym, value);
    world.row_pseudonym = Some(pseudonym);
}

#[then(regex = r#"^les chemins pseudonymisés sont exactement "([^"]*)"$"#)]
async fn flagged_paths_are(world: &mut AnonymizationWorld, paths: String) {
    let flagged: Vec<String> = anonymized(world).pseudonymized.keys().cloned().collect();
    assert_eq!(flagged, listed(&paths));
}

#[then(regex = r#"^le chemin "([^"]+)" est signalé "([^"]+)" pour la raison "([^"]+)"$"#)]
async fn path_is_flagged(world: &mut AnonymizationWorld, path: String, kind: String, why: String) {
    let flag = anonymized(world)
        .pseudonymized
        .get(&path)
        .unwrap_or_else(|| panic!("{path} is not flagged"));
    assert_eq!(flag.kind, kind);
    assert_eq!(
        serde_json::to_value(flag.why).expect("a reason"),
        Value::String(why)
    );
}

#[then(regex = r#"^le champ "([^"]+)" reste en clair$"#)]
async fn field_stays_clear(world: &mut AnonymizationWorld, path: String) {
    let (input, output) = read_field(world, &path);
    assert_eq!(output, input);
}

#[then(regex = r#"^le champ "([^"]+)" est pseudonymisé$"#)]
async fn field_is_pseudonymized(world: &mut AnonymizationWorld, path: String) {
    let (input, output) = read_field(world, &path);
    assert_ne!(output, input);
}

#[then(regex = r#"^le champ "([^"]+)" est une autre date$"#)]
async fn field_is_another_date(world: &mut AnonymizationWorld, path: String) {
    let (input, output) = read_field(world, &path);
    assert!(matches!(output, Node::Date(_)), "{output:?}");
    assert_ne!(output, input);
}

#[then(regex = r#"^les clés du champ "([^"]+)" sont les pseudonymes de "([^"]+)"$"#)]
async fn keys_are_pseudonyms(world: &mut AnonymizationWorld, path: String, values: String) {
    let anonymize = world.use_cases().anonymize_free_text;
    let expected: BTreeSet<String> = listed(&values)
        .iter()
        .map(|value| anonymize.execute(value, &[]))
        .collect();
    let (_, output) = read_field(world, &path);
    let Node::Object(entries) = output else {
        panic!("{path} is not an object: {output:?}");
    };
    let keys: BTreeSet<String> = entries.iter().map(|(key, _)| key.clone()).collect();
    assert_eq!(keys, expected);
}

#[then(regex = r#"^le champ "([^"]+)" porte le pseudonyme de la ligne$"#)]
async fn field_carries_row_pseudonym(world: &mut AnonymizationWorld, path: String) {
    let pseudonym = world.row_pseudonym.clone().expect("a row was read");
    let (_, output) = read_field(world, &path);
    assert_eq!(output, &Node::Text(pseudonym));
}

#[then(regex = r#"^la liste compte (\d+) chemins pseudonymisés et (\d+) chemins omis$"#)]
async fn flagged_paths_are_capped(world: &mut AnonymizationWorld, kept: usize, omitted: usize) {
    let anonymized = anonymized(world);
    assert_eq!(anonymized.pseudonymized.len(), kept);
    assert_eq!(anonymized.omitted_paths, omitted);
}

#[then(regex = r#"^la réponse ne contient aucune des valeurs "([^"]+)"$"#)]
async fn response_has_no_real_value(world: &mut AnonymizationWorld, values: String) {
    let anonymized = anonymized(world);
    let response = format!("{:?} {:?}", anonymized.documents, anonymized.pseudonymized);
    for value in listed(&values) {
        for form in [value.clone(), value.to_lowercase(), value.to_uppercase()] {
            assert!(!response.contains(&form), "{form} leaked in {response}");
        }
    }
}
