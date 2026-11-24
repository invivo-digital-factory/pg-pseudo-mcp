use cucumber::gherkin::Step;
use cucumber::{then, when};
use scarif_database_mcp::contexts::querying::domain::services::mongo_guard::inspect_aggregate;
use scarif_database_mcp::contexts::querying::domain::services::mongo_lineage::{
    DocumentLineage, clearance, document_lineage,
};
use scarif_database_mcp::contexts::querying::domain::types::{
    AggregateRequest, FieldLineage, FieldSource, LineageClearance, QueryLimits, Translations,
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
