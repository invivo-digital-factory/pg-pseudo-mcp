use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::composition::server_info::server_info;
use scarif_database_mcp::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;
use scarif_database_mcp::contexts::querying::domain::repositories::PseudonymTranslator;
use scarif_database_mcp::contexts::querying::domain::services::statement_guard::inspect;
use scarif_database_mcp::contexts::querying::domain::types::{
    Guarded, PersonalNames, PolicyDecision, Translations,
};
use scarif_database_mcp::contexts::querying::presentation::{AggregateArgs, FindArgs};
use serde_json::{Value, json};

use crate::querying_steps::support::{
    aggregate, aggregate_request, by_id, checked, column, error, field, find, find_request,
    run_aggregate, run_find,
};
use crate::querying_world::{CANARY_VALUES, PseudonymBook, QueryingWorld, refusal_text};

#[then(regex = r"^la réponse ne contient aucune valeur canari$")]
async fn answer_has_no_canary(world: &mut QueryingWorld) {
    let text = world.answer().text.to_lowercase();
    for value in CANARY_VALUES {
        assert!(!text.contains(&value.to_lowercase()), "{value} in {text}");
    }
    assert!(!text.contains("canary-mail"), "{text}");
}

#[then(regex = r#"^la colonne "([^"]+)" vaut "([^"]+)" en clair$"#)]
async fn column_is_clear(world: &mut QueryingWorld, column: String, expected: String) {
    let answer = world.answer();
    let body = answer.body();
    let index = body["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .position(|item| item["name"] == column.as_str())
        .unwrap_or_else(|| panic!("no column {column} in {}", answer.text));
    assert_eq!(
        answer.rows()[0][index],
        expected.as_str(),
        "{}",
        answer.text
    );
    assert!(
        body["pseudonymized"].get(column.as_str()).is_none(),
        "{}",
        answer.text
    );
}

#[then(regex = r#"^l'indice de l'erreur contient "([^"]+)"$"#)]
async fn error_hint_contains(world: &mut QueryingWorld, text: String) {
    let error = error(world);
    let hint = error["hint"].as_str().unwrap_or_default();
    assert!(hint.contains(&text), "{error}");
}

#[then(regex = r"^l'erreur (peut|ne peut pas) être retentée$")]
async fn error_retryable(world: &mut QueryingWorld, verdict: String) {
    let error = error(world);
    assert_eq!(error["retryable"], verdict == "peut", "{error}");
}

#[then(regex = r"^la réponse cite le pseudonyme$")]
async fn answer_quotes_pseudonym(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    let text = &world.answer().text;
    assert!(text.contains(&pseudonym), "{text}");
}

fn guard_sql(sql: &str, store: &PseudonymBook) -> Result<Guarded, String> {
    let inspected =
        inspect(sql).map_err(|error| refusal_text(error.code, error.message, error.hint))?;
    let mut translations = Translations::default();
    for literal in inspected.literals() {
        translations.record(literal, store.lookup(literal));
    }
    let rewritten = inspected
        .rewrite(&translations)
        .map_err(|error| refusal_text(error.code, error.message, error.hint))?;
    let mut personal = PersonalNames::default();
    for name in rewritten.lineage_names() {
        if DictionaryKind.execute(&name).is_some() {
            personal.insert(&name);
        }
    }
    Ok(rewritten.finish(&personal))
}

#[given(regex = r"^l'agent connecté à la base de test$")]
async fn agent_connected(world: &mut QueryingWorld) {
    world.connect_to_test_database();
}

#[given(regex = r"^une connexion PostgreSQL vers une base injoignable$")]
async fn unreachable_database(world: &mut QueryingWorld) {
    world.connect_to_unreachable_database();
}

#[when(regex = r#"^l'agent exécute la requête "(.+)"$"#)]
async fn agent_runs_query(world: &mut QueryingWorld, sql: String) {
    world.ask(&sql).await;
}

#[when(regex = r#"^la garde SQL examine la requête "(.+)"$"#)]
async fn sql_guard_examines(world: &mut QueryingWorld, sql: String) {
    let store = PseudonymBook::new(&world.pseudonyms);
    world.record_sql_guard(guard_sql(&sql, &store));
}

#[then(regex = r"^la requête réussit$")]
async fn query_succeeds(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
}

#[then(regex = r"^la réponse compte (\d+) lignes$")]
async fn answer_has_rows(world: &mut QueryingWorld, expected: usize) {
    assert_eq!(
        world.answer().rows().len(),
        expected,
        "{}",
        world.answer().text
    );
}

#[then(regex = r"^la requête est refusée parce qu'elle n'est pas en lecture seule$")]
async fn query_is_refused_as_read_only(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    assert_eq!(
        answer.body()["error"]["category"],
        "refused_read_only",
        "{}",
        answer.text
    );
}

#[then(regex = r#"^la requête est refusée par la règle "([^"]+)"$"#)]
async fn query_is_refused_by_rule(world: &mut QueryingWorld, rule: String) {
    let answer = world.answer();
    assert!(answer.refused, "{}", answer.text);
    let error = &answer.body()["error"];
    assert_eq!(error["category"], "refused_read_only", "{}", answer.text);
    assert_eq!(error["code"], rule.as_str(), "{}", answer.text);
}

#[then(regex = r"^la première cellule de la réponse vaut (\d+)$")]
async fn first_cell_equals(world: &mut QueryingWorld, expected: u64) {
    assert_eq!(world.answer().rows()[0][0], json!(expected));
}

fn docstring_json(step: &Step) -> Value {
    let text = step.docstring.as_deref().expect("a docstring");
    serde_json::from_str(text).expect("a JSON docstring")
}

#[given(regex = r"^l'agent connecté à une base MongoDB$")]
async fn agent_connected_to_mongodb(world: &mut QueryingWorld) {
    world.pseudonyms.clear();
    world.mongo = None;
}

#[when(regex = r"^l'agent lance l'agrégation suivante :$")]
async fn agent_runs_aggregate(world: &mut QueryingWorld, step: &Step) {
    run_aggregate(world, aggregate_request(docstring_json(step)));
}

#[when(regex = r"^l'agent cherche avec le filtre suivant :$")]
async fn agent_runs_find(world: &mut QueryingWorld, step: &Step) {
    run_find(world, find_request(docstring_json(step)));
}

#[then(regex = r"^la demande est acceptée$")]
async fn request_accepted(world: &mut QueryingWorld) {
    world.accepted_translations();
}

#[given(regex = r"^la connexion MongoDB de test à travers le bastion$")]
async fn mongo_connection(world: &mut QueryingWorld) {
    world.connect_to_test_mongodb();
}

#[when(regex = r#"^l'agent cherche dans "([^"]+)" le document d'identifiant "([0-9a-f]{24})"$"#)]
async fn find_by_id(world: &mut QueryingWorld, collection: String, id: String) {
    find(world, by_id(&collection, &id)).await;
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

#[then(regex = r"^la réponse compte (\d+) documents$")]
async fn document_count(world: &mut QueryingWorld, count: usize) {
    let body = checked(world);
    assert_eq!(body["document_count"], count, "{body}");
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

#[when(regex = r"^la politique de colonnes est chargée$")]
async fn policy_is_loaded(world: &mut QueryingWorld) {
    world.load_column_policy();
}

#[then(regex = r#"^le champ "([^"]+)" de "([^"]+)" est en clair$"#)]
async fn field_is_clear(world: &mut QueryingWorld, path: String, collection: String) {
    assert_eq!(
        world.policy.decide_field(&field(&collection, &path)),
        Some(&PolicyDecision::Clear)
    );
}

#[then(regex = r#"^la colonne "([^"]+)" est en clair$"#)]
async fn column_is_clear_by_policy(world: &mut QueryingWorld, qualified: String) {
    assert_eq!(
        world.policy.decide(&column(&qualified)),
        Some(&PolicyDecision::Clear)
    );
}

#[then(regex = r#"^la colonne "([^"]+)" n'est pas classée$"#)]
async fn column_is_unclassified(world: &mut QueryingWorld, qualified: String) {
    assert_eq!(world.policy.decide(&column(&qualified)), None);
}

#[then(regex = r#"^un avertissement "([^"]+)" nomme le fichier partagé$"#)]
async fn warning_names_shared_file(world: &mut QueryingWorld, code: String) {
    let info = serde_json::to_value(server_info(world.policy_warnings.clone(), Vec::new()))
        .expect("server info");
    let warnings = info["warnings"].as_array().expect("warnings array");
    assert_eq!(warnings.len(), 1, "{info}");
    assert_eq!(warnings[0]["code"], code.as_str(), "{info}");
    let message = warnings[0]["message"].as_str().expect("message");
    assert!(message.contains("team-columns.toml"), "{message}");
}

fn docstring(step: &Step) -> String {
    step.docstring
        .as_deref()
        .expect("a docstring")
        .trim_start_matches('\n')
        .to_string()
}

#[given(regex = r"^la politique de colonnes partagée suivante :$")]
async fn shared_policy(world: &mut QueryingWorld, step: &Step) {
    world.shared_policy = Some(docstring(step));
}

#[given(regex = r"^la politique de colonnes locale suivante :$")]
async fn local_policy(world: &mut QueryingWorld, step: &Step) {
    world.local_policy = Some(docstring(step));
}

#[when(regex = r#"^l'agent décrit la table "([^"]+)"$"#)]
async fn agent_describes(world: &mut QueryingWorld, table: String) {
    world.describe(&table).await;
}

#[then(regex = r"^la première cellule de la réponse est un pseudonyme$")]
async fn first_cell_is_a_pseudonym(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    let pseudonym = answer.rows()[0][0].as_str().expect("text cell").to_string();
    assert_eq!(answer.body()["pseudonymized"]["email"]["kind"], "email");
    assert!(!pseudonym.contains("canary-mail"), "{pseudonym}");
    world.pseudonym = Some(pseudonym);
}

#[then(regex = r"^le pseudonyme a été traduit en une valeur réelle côté serveur$")]
async fn pseudonym_was_translated(world: &mut QueryingWorld) {
    assert_eq!(world.answer().body()["translated_literals"], 1);
}

#[then(regex = r#"^la réponse porte l'avertissement "([^"]+)"$"#)]
async fn answer_carries_warning(world: &mut QueryingWorld, code: String) {
    let answer = world.answer();
    let body = answer.body();
    let found = body["warnings"].as_array().is_some_and(|warnings| {
        warnings
            .iter()
            .any(|warning| warning["code"] == code.as_str())
    });
    assert!(found, "{}", answer.text);
}

#[then(regex = r"^(\d+) valeurs? (?:a|ont) été traduites?$")]
async fn values_translated(world: &mut QueryingWorld, count: usize) {
    assert_eq!(world.accepted_translations().0, count);
}
