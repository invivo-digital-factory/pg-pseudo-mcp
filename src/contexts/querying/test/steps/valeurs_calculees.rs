use cucumber::gherkin::Step;
use cucumber::{then, when};
use scarif_database_mcp::contexts::querying::domain::services::mongo_guard::inspect_aggregate;
use scarif_database_mcp::contexts::querying::domain::services::mongo_lineage::{
    DocumentLineage, clearance, document_lineage,
};
use scarif_database_mcp::contexts::querying::domain::types::{
    AggregateRequest, FieldLineage, FieldSource, Lineage, LineageClearance, QueryLimits,
    Translations,
};
use serde_json::Value;

use crate::querying_world::QueryingWorld;

fn run(world: &mut QueryingWorld, namespace: &str, pipeline: &str) {
    let (database, collection) = namespace
        .split_once('.')
        .expect("database.collection in the scenario");
    let pipeline: Value = serde_json::from_str(pipeline).expect("a JSON pipeline");
    let request = AggregateRequest {
        connection: "MONGO".into(),
        database: Some(database.into()),
        collection: collection.into(),
        pipeline: pipeline.as_array().expect("a pipeline array").clone(),
        ..AggregateRequest::default()
    };
    let guarded = inspect_aggregate(&request)
        .unwrap_or_else(|refusal| panic!("refused: {} {}", refusal.code, refusal.message))
        .finish(&Translations::default(), &QueryLimits::default());
    world.lineage = Some(document_lineage(&guarded, database));
}

fn lineage(world: &QueryingWorld) -> &DocumentLineage {
    world.lineage.as_ref().expect("an aggregation was traced")
}

fn judged(world: &QueryingWorld, output: &str) -> LineageClearance {
    clearance(&lineage(world).of(output), &world.policy)
}

#[when(regex = r#"^l'agrégation suivante part sur "([^"]+)" :$"#)]
async fn aggregation_from_docstring(world: &mut QueryingWorld, namespace: String, step: &Step) {
    let pipeline = step.docstring.as_deref().expect("a docstring");
    run(world, &namespace, pipeline);
}

#[when(regex = r#"^l'agrégation (\[.*\]) part sur "([^"]+)"$"#)]
async fn aggregation_inline(world: &mut QueryingWorld, pipeline: String, namespace: String) {
    run(world, &namespace, &pipeline);
}

#[then(regex = r#"^le champ de sortie "([^"]+)" vient du champ "([^"]+)" de "([^"]+)"$"#)]
async fn output_comes_from(
    world: &mut QueryingWorld,
    output: String,
    path: String,
    namespace: String,
) {
    let (database, collection) = namespace
        .split_once('.')
        .expect("database.collection in the scenario");
    assert_eq!(
        lineage(world).of(&output),
        FieldLineage::Source(FieldSource {
            database: database.into(),
            collection: collection.into(),
            path,
        })
    );
}

#[then(regex = r#"^le champ de sortie "([^"]+)" est dérivé et teinté$"#)]
async fn output_is_tainted(world: &mut QueryingWorld, output: String) {
    assert_eq!(
        judged(world, &output),
        LineageClearance::Derived {
            proven_clear: false
        }
    );
}

#[then(regex = r#"^le champ de sortie "([^"]+)" est dérivé et prouvé en clair$"#)]
async fn output_is_proven_clear(world: &mut QueryingWorld, output: String) {
    assert_eq!(
        judged(world, &output),
        LineageClearance::Derived { proven_clear: true }
    );
}

#[then(regex = r#"^le champ de sortie "([^"]+)" est de lignée inconnue$"#)]
async fn output_is_unknown(world: &mut QueryingWorld, output: String) {
    assert_eq!(lineage(world).of(&output), FieldLineage::Unknown);
}

const REAL_FRAGMENTS: [&str; 4] = ["ylwenna", "kervadec", "canary-mail", "canary"];

#[then(
    regex = r#"^les colonnes (\d+) et (\d+) sont pseudonymisées comme "([^"]+)" par le dictionnaire$"#
)]
async fn columns_flagged_by_dictionary(
    world: &mut QueryingWorld,
    first: usize,
    second: usize,
    kind: String,
) {
    let answer = world.answer();
    let body = answer.body();
    for position in [first, second] {
        let flag = &body["columns"][position - 1]["pseudonymized"];
        assert_eq!(flag["kind"], kind.as_str(), "{}", answer.text);
        assert_eq!(flag["why"], "dictionary", "{}", answer.text);
    }
}

#[then(regex = r"^la colonne (\d+) n'est pas pseudonymisée$")]
async fn column_not_flagged(world: &mut QueryingWorld, position: usize) {
    let answer = world.answer();
    let body = answer.body();
    let column = &body["columns"][position - 1];
    assert!(column.is_object(), "{}", answer.text);
    assert!(column.get("pseudonymized").is_none(), "{}", answer.text);
}

#[then(regex = r"^la sortie (\d+) est de lignée (personnelle|propre|inconnue)$")]
async fn output_lineage(world: &mut QueryingWorld, position: usize, expected: String) {
    let guarded = world
        .sql
        .as_ref()
        .expect("a statement was guarded")
        .as_ref()
        .unwrap_or_else(|refusal| panic!("refused: {refusal}"));
    let expected = match expected.as_str() {
        "personnelle" => Lineage::Personal,
        "propre" => Lineage::Clean,
        _ => Lineage::Unknown,
    };
    let lineage = guarded
        .derived_lineage
        .as_ref()
        .unwrap_or_else(|| panic!("no lineage for {}", guarded.sql));
    assert_eq!(
        lineage.get(position - 1),
        Some(&expected),
        "{}",
        guarded.sql
    );
}

#[then(regex = r#"^la colonne "([^"]+)" est signalée comme dérivée et pseudonymisée$"#)]
async fn column_is_flagged_derived(world: &mut QueryingWorld, column: String) {
    let answer = world.answer();
    assert_eq!(
        answer.body()["pseudonymized"][column.as_str()]["why"],
        "derived",
        "{}",
        answer.text
    );
}

#[then(regex = r"^la réponse ne contient aucun fragment de l'adresse réelle$")]
async fn answer_has_no_real_fragment(world: &mut QueryingWorld) {
    let text = world.answer().text.to_lowercase();
    for fragment in REAL_FRAGMENTS {
        assert!(!text.contains(fragment), "{fragment} leaked in {text}");
    }
}
