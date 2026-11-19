use std::collections::HashMap;

use pg_pseudo_mcp::guard::{PseudonymLookup, prepare};
use pg_pseudo_mcp::kind::Kind;
use pg_pseudo_mcp::tools::errors::ErrorCategory;

#[derive(Default)]
struct FakeStore {
    pseudonyms: HashMap<String, (Kind, String)>,
}

impl FakeStore {
    fn with(mut self, kind: Kind, pseudonym: &str, real: &str) -> Self {
        self.pseudonyms
            .insert(pseudonym.to_string(), (kind, real.to_string()));
        self
    }
}

impl PseudonymLookup for FakeStore {
    fn reverse(&self, text: &str) -> Option<(Kind, String)> {
        self.pseudonyms.get(text).cloned()
    }

    fn looks_like_pseudonym(&self, text: &str) -> Option<Kind> {
        let domain = text.rsplit_once('@').map(|(_, domain)| domain);
        matches!(domain, Some("example.com" | "example.net" | "example.org")).then_some(Kind::Email)
    }
}

fn refusal(sql: &str) -> String {
    let error = prepare(sql, &FakeStore::default()).expect_err(sql);
    assert_eq!(error.category, ErrorCategory::RefusedReadOnly, "{sql}");
    error.code
}

#[test]
fn read_statements_pass_unchanged_in_meaning() {
    for sql in [
        "SELECT id, email FROM customers WHERE id = 1",
        "WITH recent AS (SELECT * FROM orders WHERE placed_at > now() - interval '7 days') SELECT count(*) FROM recent",
        "EXPLAIN SELECT * FROM orders WHERE customer_id = 1",
        "SELECT c.id, o.amount FROM customers c JOIN orders o ON o.customer_id = c.id WHERE c.profile->>'age' = '42'",
        "SELECT email FROM customers WHERE id = ANY(ARRAY[1, 2]) AND email ILIKE '%canary%'",
        "SELECT status::text, row_number() OVER (PARTITION BY customer_id ORDER BY placed_at) FROM orders",
        "VALUES (1, 'a')",
        "SELECT relname FROM pg_catalog.pg_class",
        "SELECT column_name FROM information_schema.columns WHERE table_name = 'orders'",
    ] {
        let guarded =
            prepare(sql, &FakeStore::default()).unwrap_or_else(|error| panic!("{sql}: {error:?}"));
        assert_eq!(guarded.translated_literals, 0, "{sql}");
    }
}

#[test]
fn writes_and_session_changes_are_refused_with_the_rule() {
    for sql in [
        "DELETE FROM orders",
        "UPDATE orders SET amount = 0",
        "INSERT INTO orders (id) VALUES (1)",
        "CREATE TABLE t (id int)",
        "DROP TABLE orders",
        "TRUNCATE orders",
        "COPY orders TO '/tmp/out.csv'",
        "SET transaction_read_only = off",
        "RESET ALL",
        "BEGIN",
        "COMMIT",
    ] {
        assert_eq!(refusal(sql), "statement_not_allowed", "{sql}");
    }
}

#[test]
fn explain_analyze_is_refused() {
    assert_eq!(
        refusal("EXPLAIN ANALYZE SELECT * FROM orders"),
        "explain_analyze"
    );
}

#[test]
fn writable_cte_is_refused() {
    assert_eq!(
        refusal("WITH d AS (DELETE FROM orders RETURNING *) SELECT * FROM d"),
        "writable_cte"
    );
}

#[test]
fn select_into_and_row_locks_are_refused() {
    assert_eq!(refusal("SELECT * INTO t FROM orders"), "select_into");
    assert_eq!(refusal("SELECT * FROM orders FOR UPDATE"), "locking_clause");
}

#[test]
fn several_statements_are_refused() {
    assert_eq!(refusal("SELECT 1; DELETE FROM orders"), "multi_statement");
}

#[test]
fn server_and_session_functions_are_refused_with_their_name() {
    for (sql, function) in [
        (
            "SELECT set_config('transaction_read_only', 'off', false)",
            "set_config",
        ),
        (
            "SELECT pg_catalog.pg_terminate_backend(1)",
            "pg_terminate_backend",
        ),
        ("SELECT nextval('audit_seq')", "nextval"),
        ("SELECT pg_stat_reset()", "pg_stat_reset"),
        ("SELECT pg_switch_wal()", "pg_switch_wal"),
        (
            "SELECT pg_create_restore_point('x')",
            "pg_create_restore_point",
        ),
        ("SELECT pg_sleep(10)", "pg_sleep"),
        ("SELECT lo_import('/etc/passwd')", "lo_import"),
        (
            "SELECT * FROM dblink('host=x', 'select 1') AS t(a int)",
            "dblink",
        ),
    ] {
        let error = prepare(sql, &FakeStore::default()).expect_err(sql);
        assert_eq!(error.code, "function_not_allowed", "{sql}");
        assert!(error.message.contains(function), "{sql}: {}", error.message);
    }
}

#[test]
fn data_bearing_catalogs_are_refused_with_a_hint() {
    for sql in [
        "SELECT * FROM pg_stats",
        "SELECT query FROM pg_catalog.pg_stat_activity",
        "SELECT * FROM pg_stats_ext",
        "SELECT * FROM pg_user_mappings",
        "SELECT * FROM pg_stat_get_activity(NULL)",
        "SELECT * FROM pg_settings",
    ] {
        let error = prepare(sql, &FakeStore::default()).expect_err(sql);
        assert!(
            matches!(
                error.code.as_str(),
                "relation_not_allowed" | "function_not_allowed"
            ),
            "{sql}: {}",
            error.code
        );
        assert!(
            error
                .hint
                .as_deref()
                .unwrap_or_default()
                .contains("describe_table"),
            "{sql}"
        );
    }
}

#[test]
fn unparseable_sql_is_refused_with_the_parser_message() {
    let error = prepare("SELEC id FROM", &FakeStore::default()).expect_err("parse error");
    assert_eq!(error.code, "parse_error");
    assert!(!error.message.is_empty());
}

#[test]
fn a_known_pseudonym_is_translated_to_the_real_value() {
    let store = FakeStore::default().with(
        Kind::Email,
        "claire.martin@example.net",
        "ilona.kervadec@canary-mail.fr",
    );
    let guarded = prepare(
        "SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = 'claire.martin@example.net'",
        &store,
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 1);
    assert!(
        guarded.sql.contains("'ilona.kervadec@canary-mail.fr'"),
        "{}",
        guarded.sql
    );
    assert!(!guarded.sql.contains("claire.martin"));
    assert_eq!(
        guarded.injected,
        vec![(
            "ilona.kervadec@canary-mail.fr".to_string(),
            "claire.martin@example.net".to_string()
        )]
    );
}

#[test]
fn a_pseudonym_inside_a_like_pattern_is_translated_and_escaped() {
    let store = FakeStore::default().with(
        Kind::Email,
        "claire.martin@example.net",
        "ilo_na%k@canary-mail.fr",
    );
    let guarded = prepare(
        "SELECT id FROM customers WHERE email ILIKE '%claire.martin@example.net%'",
        &store,
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 1);
    assert!(
        guarded.sql.contains(r"'%ilo\_na\%k@canary-mail.fr%'"),
        "{}",
        guarded.sql
    );
}

#[test]
fn a_numeric_pseudonym_is_translated() {
    let store = FakeStore::default().with(Kind::Phone, "33639981234", "33641278395");
    let guarded = prepare(
        "SELECT id FROM customers WHERE telephone_secours = 33639981234",
        &store,
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 1);
    assert!(guarded.sql.contains("33641278395"), "{}", guarded.sql);
}

#[test]
fn a_real_value_with_quotes_cannot_change_the_statement() {
    let store = FakeStore::default().with(
        Kind::PersonName,
        "Kalin Torvo",
        "x'); DELETE FROM orders; -- \\ $$",
    );
    let guarded = prepare(
        "SELECT id FROM orders WHERE titulaire = 'Kalin Torvo'",
        &store,
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 1);
    let reparsed = sqlparser::parser::Parser::parse_sql(
        &sqlparser::dialect::PostgreSqlDialect {},
        &guarded.sql,
    )
    .unwrap();
    assert_eq!(reparsed.len(), 1);
    assert!(matches!(reparsed[0], sqlparser::ast::Statement::Query(_)));
}

#[test]
fn an_unknown_pseudonym_shaped_literal_is_left_and_warned() {
    let guarded = prepare(
        "SELECT id FROM customers WHERE email = 'nobody.known@example.net'",
        &FakeStore::default(),
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 0);
    assert!(guarded.sql.contains("nobody.known@example.net"));
    assert_eq!(guarded.warnings.len(), 1);
    assert_eq!(guarded.warnings[0].code, "unresolved_pseudonym");
}

#[test]
fn the_explain_form_wraps_a_validated_query() {
    let guarded = prepare("SELECT * FROM orders", &FakeStore::default()).unwrap();
    assert!(guarded.explain_sql().starts_with("EXPLAIN"));
    let error = prepare("EXPLAIN SELECT 1", &FakeStore::default()).unwrap();
    assert!(error.explain_sql().matches("EXPLAIN").count() == 1);
}
