mod common;

use common::{CONNECTION, Harness, assert_no_canary, is_error, text_of};
use pg_pseudo_mcp::tools::query::{QueryArgs, query};
use serde_json::Value;

async fn ask(harness: &Harness, sql: &str) -> (bool, String) {
    let result = query(
        &harness.state,
        QueryArgs {
            connection: CONNECTION.to_string(),
            sql: sql.to_string(),
            ..QueryArgs::default()
        },
    )
    .await;
    (is_error(&result), text_of(&result))
}

const MUST_BE_REFUSED: &[&str] = &[
    "SELECT xmlelement(name e, email) FROM customers",
    "SELECT * FROM pg_stats WHERE tablename = 'customers'",
    "SELECT most_common_vals FROM pg_catalog.pg_stats",
    "SELECT query FROM pg_stat_activity",
    "COPY customers TO STDOUT",
    "SELECT set_config('statement_timeout', '0', false)",
    "WITH x AS (UPDATE customers SET nom = 'x' RETURNING email) SELECT * FROM x",
];

const MUST_SUCCEED_PSEUDONYMIZED: &[&str] = &[
    "SELECT email, nom, prenom, telephone, iban, siren, nir FROM customers",
    "SELECT * FROM customers",
    "SELECT upper(email), lower(nom), initcap(prenom) FROM customers",
    "SELECT reverse(email) FROM customers",
    "SELECT replace(email, '@', ' at ') FROM customers",
    "SELECT split_part(email, '@', 1) FROM customers",
    "SELECT left(email, 12), right(email, 14) FROM customers",
    "WITH x AS (SELECT email AS e FROM customers) SELECT split_part(e, '@', 1) FROM x",
    "SELECT substr(nom, 1, 4) || substr(prenom, 2, 3) FROM customers",
    "SELECT split_part(unnest(emails_secondaires), '@', 1) FROM customers",
    "SELECT split_part(profile->'contact'->>'mail', '@', 1) FROM customers",
    "SELECT translate(nom, 'aeiou', 'AEIOU') FROM customers",
    "SELECT nom || '-' || prenom FROM customers",
    "SELECT format('%s %s', prenom, nom) FROM customers",
    "SELECT md5(email), sha256(convert_to(email, 'UTF8')) FROM customers",
    "SELECT encode(convert_to(email, 'UTF8'), 'hex') FROM customers",
    "SELECT encode(convert_to(email, 'UTF8'), 'base64') FROM customers",
    "SELECT encode(convert_to(nom, 'UTF8'), 'base64'), encode(convert_to(prenom, 'UTF8'), 'hex'), to_hex(telephone_secours), encode(convert_to(email, 'UTF8'), 'hex') FROM customers",
    "SELECT upper(encode(convert_to(email, 'UTF8'), 'hex')), convert_to(nom, 'UTF8')::text FROM customers",
    "SELECT to_jsonb(c) FROM customers c",
    "SELECT row_to_json(o) FROM orders o",
    "SELECT array_agg(email) FROM customers",
    "SELECT string_agg(nom, ' ') FROM customers",
    "SELECT jsonb_agg(profile) FROM customers",
    "SELECT profile->'contact'->>'mail' FROM customers",
    "SELECT jsonb_object_keys(profile) FROM customers WHERE profile IS NOT NULL",
    "SELECT unnest(emails_secondaires) FROM customers",
    "SELECT body FROM notes",
    "SELECT regexp_replace(body, '[0-9]', '#', 'g') FROM notes",
    "SELECT telephone_secours::text, telephone_secours + 0 FROM customers",
    "SELECT date_naissance::date AS d, date_naissance + 1 FROM customers",
    "SELECT telephone_secours * 1.0, telephone_secours::float8 FROM customers",
    "SELECT siren::bigint, replace(iban, ' ', '') FROM customers",
];

const MAY_ERROR: &[&str] = &[
    "SELECT email::int FROM customers",
    "SELECT nom::int FROM customers",
    "SELECT (nom || 'x')::date FROM customers",
    "SELECT 1/0 FROM customers WHERE email = 'x'",
];

#[tokio::test]
#[ignore]
async fn write_and_side_effect_statements_are_refused_as_read_only() {
    let harness = Harness::new();
    for sql in MUST_BE_REFUSED {
        let (error, text) = ask(&harness, sql).await;
        assert_no_canary(&text);
        assert!(error, "{sql} should be refused: {text}");
        let body: Value = serde_json::from_str(&text).expect("json error body");
        assert_eq!(
            body["error"]["category"], "refused_read_only",
            "{sql}: {text}"
        );
    }
}

#[tokio::test]
#[ignore]
async fn hostile_reads_succeed_without_any_real_value() {
    let harness = Harness::new();
    for sql in MUST_SUCCEED_PSEUDONYMIZED {
        let (error, text) = ask(&harness, sql).await;
        assert_no_canary(&text);
        assert!(!error, "{sql} should succeed: {text}");
    }
}

#[tokio::test]
#[ignore]
async fn failing_casts_and_arithmetic_return_no_real_value() {
    let harness = Harness::new();
    for sql in MAY_ERROR {
        let (_, text) = ask(&harness, sql).await;
        assert_no_canary(&text);
    }
}

#[tokio::test]
#[ignore]
async fn character_level_probing_is_a_documented_limit() {
    let harness = Harness::new();
    let (error, text) = ask(
        &harness,
        "SELECT ascii(substr(email, 1, 1)) AS first_code FROM customers WHERE id = 1",
    )
    .await;
    assert!(!error, "{text}");
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["rows"][0][0], Value::from(i64::from(b'y')));

    let (_, text) = ask(
        &harness,
        "SELECT count(*) FROM customers WHERE email LIKE 'ylwenna%'",
    )
    .await;
    let response: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(response["rows"][0][0], Value::from(1));
}
