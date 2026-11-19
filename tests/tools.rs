mod common;

use common::{CONNECTION, Harness, assert_no_canary, body, is_error, text_of};
use pg_pseudo_mcp::tools::explain::{ExplainArgs, explain};
use pg_pseudo_mcp::tools::query::{QueryArgs, query};
use serde_json::{Value, json};

async fn run(harness: &Harness, sql: &str) -> (bool, String) {
    run_with(harness, sql, None).await
}

async fn run_with(harness: &Harness, sql: &str, max_rows: Option<u64>) -> (bool, String) {
    run_args(
        harness,
        QueryArgs {
            connection: CONNECTION.to_string(),
            sql: sql.to_string(),
            max_rows,
            ..QueryArgs::default()
        },
    )
    .await
}

async fn run_args(harness: &Harness, args: QueryArgs) -> (bool, String) {
    let result = query(&harness.state, args).await;
    let text = text_of(&result);
    assert_no_canary(&text);
    (is_error(&result), text)
}

fn rows(text: &str) -> Vec<Vec<Value>> {
    serde_json::from_str::<Value>(text).unwrap()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row.as_array().unwrap().clone())
        .collect()
}

#[tokio::test]
#[ignore]
async fn a_customer_can_be_followed_across_queries_through_its_pseudonym() {
    let harness = Harness::new();
    let (error, first) = run(&harness, "SELECT email FROM customers WHERE id = 1").await;
    assert!(!error, "{first}");
    let pseudonym = rows(&first)[0][0].as_str().unwrap().to_string();
    let response: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(response["pseudonymized"]["email"]["kind"], "email");

    let (error, second) = run(
        &harness,
        &format!(
            "SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = '{pseudonym}' ORDER BY o.id"
        ),
    )
    .await;
    assert!(!error, "{second}");
    let response: Value = serde_json::from_str(&second).unwrap();
    assert_eq!(response["translated_literals"], 1);
    assert_eq!(response["rows"], json!([[10], [11]]));
}

#[tokio::test]
#[ignore]
async fn the_same_value_gets_the_same_pseudonym_in_every_table() {
    let harness = Harness::new();
    let (_, from_customers) = run(&harness, "SELECT email FROM customers WHERE id = 2").await;
    let (_, from_notes) = run(&harness, "SELECT body FROM notes WHERE id = 101").await;
    let pseudonym = rows(&from_customers)[0][0].as_str().unwrap().to_string();
    assert!(from_notes.contains(&pseudonym), "{from_notes}");
}

#[tokio::test]
#[ignore]
async fn derived_and_serialized_values_never_expose_real_ones() {
    let harness = Harness::new();
    for sql in [
        "SELECT lower(email), upper(nom) FROM customers",
        "SELECT to_json(c) FROM customers c",
        "SELECT c FROM customers c",
        "SELECT string_agg(email, ',') FROM customers",
        "SELECT nom || ' ' || prenom AS full_name FROM customers",
        "SELECT md5(email), encode(convert_to(email, 'UTF8'), 'base64') FROM customers",
        "SELECT profile, emails_secondaires FROM customers",
        "SELECT titulaire FROM orders",
        "SELECT body FROM notes",
    ] {
        let (error, text) = run(&harness, sql).await;
        assert!(!error, "{sql}: {text}");
        let response: Value = serde_json::from_str(&text).unwrap();
        assert!(
            response["pseudonymized"]
                .as_object()
                .is_some_and(|map| !map.is_empty()),
            "{sql}: {text}"
        );
    }
    let (_, text) = run(&harness, "SELECT lower(email) AS e FROM customers").await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["pseudonymized"]["e"]["why"], "derived");
}

#[tokio::test]
#[ignore]
async fn free_text_keeps_its_structure() {
    let harness = Harness::new();
    let (_, text) = run(&harness, "SELECT body FROM notes WHERE id = 100").await;
    let note = rows(&text)[0][0].as_str().unwrap().to_string();
    assert!(note.starts_with("Appel de M. "), "{note}");
    assert!(note.contains(" au "), "{note}");
    assert!(note.contains(", livraison "), "{note}");
}

#[tokio::test]
#[ignore]
async fn technical_columns_pass_in_clear_and_personal_numbers_do_not() {
    let harness = Harness::new();
    let (_, text) = run(
        &harness,
        "SELECT c.id, o.status, o.status_label, o.amount, c.telephone_secours, c.avatar, o.titulaire FROM orders o JOIN customers c ON c.id = o.customer_id WHERE o.id IN (10, 13) ORDER BY o.id",
    )
    .await;
    let rows = rows(&text);
    assert_eq!(rows[0][0], json!(1));
    assert_eq!(rows[0][1], json!("paid"));
    assert_eq!(rows[0][2], json!("PAID"));
    assert_eq!(rows[0][3], json!("12345.6700"));
    assert!(rows[0][4].is_number(), "{text}");
    assert_ne!(rows[0][4], json!(33641278395_i64));
    assert!(rows[0][5].as_str().unwrap().starts_with("<bytea"), "{text}");
    assert_eq!(rows[1][4], Value::Null);
    assert_eq!(rows[1][6], json!(""));
}

#[tokio::test]
#[ignore]
async fn a_pseudonym_outside_a_comparison_stays_a_pseudonym() {
    let harness = Harness::new();
    let (_, first) = run(&harness, "SELECT email FROM customers WHERE id = 1").await;
    let pseudonym = rows(&first)[0][0].as_str().unwrap().to_string();
    let (_, text) = run(
        &harness,
        &format!("SELECT '{pseudonym}' AS x, substr('{pseudonym}', 1, 6) AS y"),
    )
    .await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["translated_literals"], 0);
    assert_eq!(response["warnings"][0]["code"], "pseudonym_not_translated");
}

#[tokio::test]
#[ignore]
async fn errors_quote_the_pseudonym_not_the_real_value() {
    let harness = Harness::new();
    let (_, first) = run(&harness, "SELECT email FROM customers WHERE id = 1").await;
    let pseudonym = rows(&first)[0][0].as_str().unwrap().to_string();
    let (error, text) = run(
        &harness,
        &format!("SELECT id FROM customers WHERE id = '{pseudonym}'::int"),
    )
    .await;
    assert!(error, "{text}");
    assert!(text.contains("sql_error"), "{text}");
    assert!(text.contains(&pseudonym), "{text}");
}

#[tokio::test]
#[ignore]
async fn an_unknown_pseudonym_is_reported_instead_of_an_empty_answer() {
    let harness = Harness::new();
    let (_, text) = run(
        &harness,
        "SELECT id FROM customers WHERE email = 'nobody.here@example.net'",
    )
    .await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["row_count"], 0);
    assert_eq!(response["translated_literals"], 0);
    assert_eq!(response["warnings"][0]["code"], "unresolved_pseudonym");
}

#[tokio::test]
#[ignore]
async fn writes_are_refused_and_change_nothing() {
    let harness = Harness::new();
    let (error, text) = run(&harness, "DELETE FROM orders").await;
    assert!(error, "{text}");
    assert!(text.contains("refused_read_only"), "{text}");
    let (_, count) = run(&harness, "SELECT count(*) FROM orders").await;
    assert_eq!(rows(&count)[0][0], json!(4));
}

#[tokio::test]
#[ignore]
async fn results_are_capped_with_a_structured_truncation() {
    let harness = Harness::new();
    let (_, text) = run(&harness, "SELECT id FROM big_numbers ORDER BY id").await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["row_count"], 50);
    assert_eq!(response["truncated"][0]["by"], "rows");

    let (_, text) = run_with(
        &harness,
        "SELECT id FROM big_numbers ORDER BY id",
        Some(900),
    )
    .await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["row_count"], 500);
    assert_eq!(response["warnings"][0]["code"], "max_rows_clamped");

    let (_, text) = run(
        &harness,
        "SELECT string_agg(c.email, ',') AS emails FROM customers c, generate_series(1, 40)",
    )
    .await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert!(text.len() < 70_000);
    assert_eq!(response["truncated"][0]["by"], "cell");
    assert_eq!(response["truncated"][0]["column"], "emails");
}

#[tokio::test]
#[ignore]
async fn explain_shows_the_pseudonym_in_the_plan() {
    let harness = Harness::new();
    let (_, first) = run(&harness, "SELECT email FROM customers WHERE id = 1").await;
    let pseudonym = rows(&first)[0][0].as_str().unwrap().to_string();
    let result = explain(
        &harness.state,
        ExplainArgs {
            connection: CONNECTION.to_string(),
            sql: format!("SELECT * FROM customers WHERE email = '{pseudonym}'"),
        },
    )
    .await;
    let text = text_of(&result);
    assert_no_canary(&text);
    let response = body(&result);
    assert_eq!(response["translated_literals"], 1);
    assert!(text.contains(&pseudonym), "{text}");
}

#[tokio::test]
#[ignore]
async fn an_unknown_connection_is_a_structured_error() {
    let harness = Harness::new();
    let result = query(
        &harness.state,
        QueryArgs {
            connection: "nope".into(),
            sql: "SELECT 1".into(),
            ..QueryArgs::default()
        },
    )
    .await;
    assert!(is_error(&result));
    assert!(text_of(&result).contains("unknown_connection"));
}

#[tokio::test]
#[ignore]
async fn a_long_cell_is_read_in_contiguous_pseudonymized_slices() {
    let harness = Harness::new();
    let (_, single) = run(&harness, "SELECT body FROM notes WHERE id = 100").await;
    let note = rows(&single)[0][0].as_str().unwrap().to_string();
    let phone = note
        .split_once(" au ")
        .and_then(|(_, rest)| rest.split_once(','))
        .map(|(phone, _)| phone.to_string())
        .unwrap();
    let sql = "SELECT string_agg(body, ' / ') AS notes FROM notes, generate_series(1, 20) WHERE notes.id = 100";
    let mut whole = String::new();
    let mut total = None;
    let mut offset = None;
    loop {
        let (error, text) = run_args(
            &harness,
            QueryArgs {
                connection: CONNECTION.to_string(),
                sql: sql.to_string(),
                cell_offset: offset,
                ..QueryArgs::default()
            },
        )
        .await;
        assert!(!error, "{text}");
        let cell = rows(&text)[0][0].as_str().unwrap().to_string();
        let (kept, rest) = match cell.split_once("…[+") {
            Some((kept, rest)) => (kept.to_string(), Some(rest.to_string())),
            None => (cell, None),
        };
        if total.is_none() {
            let remaining: usize = rest
                .as_deref()
                .and_then(|rest| rest.strip_suffix(" chars]"))
                .unwrap()
                .parse()
                .unwrap();
            total = Some(kept.chars().count() + remaining);
        }
        whole.push_str(&kept);
        let response: Value = serde_json::from_str(&text).unwrap();
        let Some(cut) = response["truncated"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["by"] == "cell").cloned())
        else {
            break;
        };
        let hint = cut["hint"].as_str().unwrap();
        assert!(!hint.contains("substr"), "{hint}");
        let next = offset.unwrap_or(0) + cut["limit"].as_u64().unwrap();
        assert!(hint.contains(&format!("cell_offset={next}")), "{hint}");
        offset = Some(next);
    }
    assert!(offset.is_some(), "the aggregate must span several slices");
    assert_no_canary(&whole);
    assert_eq!(Some(whole.chars().count()), total, "{whole}");
    assert!(
        whole.starts_with(&note[..note.find(&phone).unwrap()]),
        "{whole}"
    );
    assert_eq!(whole.matches(&phone).count(), 20, "{whole}");
}

#[tokio::test]
#[ignore]
async fn the_same_column_name_twice_keeps_one_flag_per_column() {
    let harness = Harness::new();
    let (error, text) = run(
        &harness,
        "SELECT a.email, b.email, a.id FROM customers a JOIN customers b ON a.id = b.id ORDER BY a.id",
    )
    .await;
    assert!(!error, "{text}");
    let response: Value = serde_json::from_str(&text).unwrap();
    let columns = response["columns"].as_array().unwrap();
    for column in &columns[..2] {
        assert_eq!(column["pseudonymized"]["kind"], "email", "{text}");
        assert_eq!(column["pseudonymized"]["why"], "dictionary", "{text}");
    }
    assert!(columns[2].get("pseudonymized").is_none(), "{text}");
    assert_eq!(response["pseudonymized"]["email"]["kind"], "email");
}

#[tokio::test]
#[ignore]
async fn a_larger_byte_budget_can_be_requested_up_to_the_ceiling() {
    let harness = Harness::new();
    let sql = "SELECT id, array_fill(7, ARRAY[200]) AS pad FROM big_numbers ORDER BY id";
    let (_, text) = run(&harness, sql).await;
    let default: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(default["truncated"][0]["by"], "bytes", "{text}");
    assert_eq!(default["truncated"][0]["limit"], 16384);
    let (_, text) = run_args(
        &harness,
        QueryArgs {
            connection: CONNECTION.to_string(),
            sql: sql.to_string(),
            max_rows: Some(500),
            max_bytes: Some(1_000_000),
            ..QueryArgs::default()
        },
    )
    .await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["truncated"][0]["limit"], 65536);
    assert!(
        response["row_count"].as_u64().unwrap() > default["row_count"].as_u64().unwrap(),
        "{text}"
    );
    assert_eq!(response["warnings"][0]["code"], "max_bytes_clamped");
}

#[tokio::test]
#[ignore]
async fn explain_in_json_format_returns_a_plan() {
    let harness = Harness::new();
    let (_, first) = run(&harness, "SELECT email FROM customers WHERE id = 1").await;
    let pseudonym = rows(&first)[0][0].as_str().unwrap().to_string();
    for format in ["JSON", "YAML"] {
        let result = explain(
            &harness.state,
            ExplainArgs {
                connection: CONNECTION.to_string(),
                sql: format!(
                    "EXPLAIN (FORMAT {format}) SELECT * FROM customers WHERE email = '{pseudonym}'"
                ),
            },
        )
        .await;
        let text = text_of(&result);
        assert_no_canary(&text);
        let response = body(&result);
        let plan = response["plan"].as_array().unwrap();
        assert!(plan.len() > 1, "{format}: {text}");
        assert!(text.contains(&pseudonym), "{format}: {text}");
        assert!(response.get("truncated").is_none(), "{format}: {text}");
    }
}
