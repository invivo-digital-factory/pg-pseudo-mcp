use cucumber::{then, when};
use scarif_database_mcp::composition::server_info::server_info;
use serde_json::{Value, json};

use crate::querying_world::{Answer, QueryingWorld};

fn info(world: &QueryingWorld) -> Value {
    world.answer().body()
}

fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .expect("a list")
        .iter()
        .map(|item| item.as_str().unwrap_or_default().to_string())
        .collect()
}

#[when(regex = r"^l'agent demande les informations du serveur$")]
async fn agent_asks_server_info(world: &mut QueryingWorld) {
    let info = server_info(world.policy_warnings.clone(), Vec::new());
    world.answer = Some(Answer {
        refused: false,
        text: serde_json::to_string(&info).expect("serializable server info"),
    });
}

#[then(regex = r#"^les moteurs annoncés sont "([^"]+)" et "([^"]+)"$"#)]
async fn engines(world: &mut QueryingWorld, first: String, second: String) {
    assert_eq!(info(world)["engines"], json!([first, second]));
}

#[then(regex = r"^les (limites par défaut|plafonds) sont (\d+) lignes, (\d+) octets et (\d+) ms$")]
async fn limits(world: &mut QueryingWorld, which: String, rows: u64, bytes: u64, timeout: u64) {
    let key = if which == "plafonds" {
        "maxima"
    } else {
        "defaults"
    };
    assert_eq!(
        info(world)[key],
        json!({"max_rows": rows, "max_bytes": bytes, "timeout_ms": timeout})
    );
}

#[then(regex = r"^une valeur texte est coupée à (\d+) caractères$")]
async fn cell_cap(world: &mut QueryingWorld, characters: u64) {
    assert_eq!(info(world)["max_cell_chars"], characters);
}

#[then(regex = r#"^le type "([^"]+)" est annoncé (réversible|non réversible)$"#)]
async fn kind_reversibility(world: &mut QueryingWorld, kind: String, verdict: String) {
    let info = info(world);
    let reversible = names(&info["reversible_kinds"]).contains(&kind);
    let non_reversible = names(&info["non_reversible_kinds"]).contains(&kind);
    assert_eq!(reversible, verdict == "réversible", "{info}");
    assert_eq!(non_reversible, verdict != "réversible", "{info}");
}

#[then(
    regex = r#"^les étapes MongoDB annoncées comptent (\d+) étapes dont "([^"]+)", "([^"]+)" et "([^"]+)"$"#
)]
async fn mongo_stages(
    world: &mut QueryingWorld,
    count: usize,
    first: String,
    second: String,
    third: String,
) {
    let stages = names(&info(world)["mongodb"]["stages"]);
    assert_eq!(stages.len(), count, "{stages:?}");
    for stage in [first, second, third] {
        assert!(stages.contains(&stage), "{stage} not in {stages:?}");
    }
}

#[then(
    regex = r#"^les étapes MongoDB annoncées ne contiennent ni "([^"]+)", ni "([^"]+)", ni "([^"]+)"$"#
)]
async fn mongo_stages_exclude(
    world: &mut QueryingWorld,
    first: String,
    second: String,
    third: String,
) {
    let info = info(world);
    let mut allowed = names(&info["mongodb"]["stages"]);
    allowed.extend(names(&info["mongodb"]["query_operators"]));
    allowed.extend(names(&info["mongodb"]["expression_operators"]));
    for name in [first, second, third] {
        assert!(!allowed.contains(&name), "{name} is allowed");
    }
}

#[then(regex = r#"^"([^"]+)" est annoncé seulement en première étape$"#)]
async fn first_stage_only(world: &mut QueryingWorld, stage: String) {
    assert_eq!(info(world)["mongodb"]["first_stage_only"], json!([stage]));
}

#[then(
    regex = r"^le plafond annoncé est de (\d+) étapes, (\d+) niveaux, (\d+) octets et (\d+) éléments par tableau$"
)]
async fn mongo_caps(world: &mut QueryingWorld, stages: u64, depth: u64, bytes: u64, items: u64) {
    let mongo = &info(world)["mongodb"];
    assert_eq!(mongo["max_pipeline_stages"], stages, "{mongo}");
    assert_eq!(mongo["max_nesting_depth"], depth, "{mongo}");
    assert_eq!(mongo["max_request_bytes"], bytes, "{mongo}");
    assert_eq!(mongo["max_request_array_elements"], items, "{mongo}");
}

#[then(regex = r"^les informations ne portent aucun avertissement$")]
async fn no_warning(world: &mut QueryingWorld) {
    let info = info(world);
    assert!(info.get("warnings").is_none(), "{info}");
}
