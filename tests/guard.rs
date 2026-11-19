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
        "ylwenna.kervadec@canary-mail.fr",
    );
    let guarded = prepare(
        "SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = 'claire.martin@example.net'",
        &store,
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 1);
    assert!(
        guarded.sql.contains("'ylwenna.kervadec@canary-mail.fr'"),
        "{}",
        guarded.sql
    );
    assert!(!guarded.sql.contains("claire.martin"));
    assert_eq!(
        guarded.injected,
        vec![(
            "ylwenna.kervadec@canary-mail.fr".to_string(),
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
fn the_table_shorthand_cannot_reach_a_refused_catalog() {
    for sql in [
        "TABLE pg_stats",
        "TABLE pg_catalog.pg_authid",
        "SELECT 1 UNION TABLE pg_stats",
    ] {
        let code = refusal(sql);
        assert!(
            matches!(code.as_str(), "relation_not_allowed" | "parse_error"),
            "{sql}: {code}"
        );
    }
}

#[test]
fn explain_with_the_analyze_option_is_refused() {
    assert_eq!(
        refusal("EXPLAIN (ANALYZE) SELECT * FROM orders"),
        "explain_analyze"
    );
    assert_eq!(
        refusal("EXPLAIN (FORMAT TEXT, ANALYZE true) SELECT * FROM orders"),
        "explain_analyze"
    );
    assert!(
        prepare(
            "EXPLAIN (ANALYZE false) SELECT * FROM orders",
            &FakeStore::default()
        )
        .is_ok()
    );
}

#[test]
fn settings_and_foreign_credentials_are_refused() {
    assert_eq!(
        refusal("SELECT * FROM information_schema.user_mapping_options"),
        "relation_not_allowed"
    );
    assert_eq!(
        refusal("SELECT current_setting('my.secret')"),
        "function_not_allowed"
    );
}

#[test]
fn a_pseudonym_outside_a_comparison_is_not_translated() {
    let store = FakeStore::default().with(
        Kind::Email,
        "claire.martin@example.net",
        "ylwenna.kervadec@canary-mail.fr",
    );
    for sql in [
        "SELECT 'claire.martin@example.net' AS x",
        "SELECT substr('claire.martin@example.net', 1, 6)",
        "SELECT md5('claire.martin@example.net') FROM customers",
        "SELECT CASE WHEN 'claire.martin@example.net' LIKE 'i%' THEN 1 END",
        "SELECT 'claire.martin@example.net' = 'someone@else.fr'",
        "SELECT id FROM customers WHERE email > 'claire.martin@example.net'",
        "SELECT id FROM customers WHERE email BETWEEN 'claire.martin@example.net' AND 'z'",
        "SELECT id FROM customers WHERE lower(email) = 'claire.martin@example.net'",
    ] {
        let guarded = prepare(sql, &store).unwrap();
        assert_eq!(guarded.translated_literals, 0, "{sql}");
        assert!(!guarded.sql.contains("ylwenna"), "{sql}: {}", guarded.sql);
        assert_eq!(
            guarded.warnings[0].code, "pseudonym_not_translated",
            "{sql}"
        );
    }
}

#[test]
fn json_fields_compared_with_a_pseudonym_are_translated() {
    let store = FakeStore::default().with(
        Kind::Email,
        "claire.martin@example.net",
        "o.hazvekmor@canary-mail.fr",
    );
    let guarded = prepare(
        "SELECT id FROM customers WHERE profile->'contact'->>'mail' = 'claire.martin@example.net'",
        &store,
    )
    .unwrap();
    assert_eq!(guarded.translated_literals, 1, "{}", guarded.sql);
}

#[test]
fn only_structural_information_schema_views_are_readable() {
    assert!(
        prepare(
            "SELECT table_name FROM information_schema.tables",
            &FakeStore::default()
        )
        .is_ok()
    );
    for sql in [
        "SELECT * FROM information_schema._pg_user_mappings",
        "SELECT * FROM information_schema.check_constraints",
        "SELECT * FROM information_schema.foreign_server_options",
    ] {
        assert_eq!(refusal(sql), "relation_not_allowed", "{sql}");
    }
}

#[test]
fn pseudonyms_in_comparisons_lists_and_casts_are_translated() {
    let store = FakeStore::default()
        .with(
            Kind::Email,
            "claire.martin@example.net",
            "ylwenna.kervadec@canary-mail.fr",
        )
        .with(
            Kind::Email,
            "paul.rivet@example.org",
            "odrigan.hazvekmor@canary-mail.fr",
        );
    for sql in [
        "SELECT id FROM customers WHERE email IN ('claire.martin@example.net', 'paul.rivet@example.org')",
        "SELECT id FROM customers WHERE email = ANY(ARRAY['claire.martin@example.net', 'paul.rivet@example.org'])",
        "SELECT id FROM customers WHERE 'claire.martin@example.net'::text = email OR email = 'paul.rivet@example.org'",
        "SELECT CASE WHEN email = 'claire.martin@example.net' THEN 1 END, 'x' FROM customers WHERE email <> 'paul.rivet@example.org'",
    ] {
        let guarded = prepare(sql, &store).unwrap();
        assert_eq!(guarded.translated_literals, 2, "{sql}: {}", guarded.sql);
        assert!(!guarded.sql.contains("example."), "{sql}: {}", guarded.sql);
    }
}

#[test]
fn the_explain_form_wraps_a_validated_query() {
    let guarded = prepare("SELECT * FROM orders", &FakeStore::default()).unwrap();
    assert!(guarded.explain_sql().starts_with("EXPLAIN"));
    let error = prepare("EXPLAIN SELECT 1", &FakeStore::default()).unwrap();
    assert!(error.explain_sql().matches("EXPLAIN").count() == 1);
}

#[test]
fn functions_called_from_the_from_clause_are_refused_with_their_name() {
    for (sql, function) in [
        (
            "SELECT * FROM LATERAL pg_terminate_backend(1) t",
            "pg_terminate_backend",
        ),
        (
            "SELECT * FROM orders o, LATERAL pg_advisory_lock(4242) l",
            "pg_advisory_lock",
        ),
        (
            "SELECT * FROM LATERAL set_config('a', 'b', false) s",
            "set_config",
        ),
        (
            "SELECT * FROM LATERAL pg_read_file('/etc/passwd') f",
            "pg_read_file",
        ),
        (
            "SELECT * FROM LATERAL dblink_exec('x', 'DELETE FROM orders') AS d(r text)",
            "dblink_exec",
        ),
        (
            "SELECT * FROM orders o CROSS JOIN LATERAL pg_catalog.pg_terminate_backend(o.id) t",
            "pg_terminate_backend",
        ),
        (
            "SELECT * FROM LATERAL generate_series(1, pg_sleep(1)::int) g",
            "pg_sleep",
        ),
        (
            "SELECT * FROM unnest(ARRAY[pg_cancel_backend(1)]) u",
            "pg_cancel_backend",
        ),
    ] {
        let error = prepare(sql, &FakeStore::default()).expect_err(sql);
        assert_eq!(error.code, "function_not_allowed", "{sql}");
        assert!(error.message.contains(function), "{sql}: {}", error.message);
    }
}

#[test]
fn ordinary_table_functions_remain_readable() {
    for sql in [
        "SELECT * FROM generate_series(1, 3) g",
        "SELECT * FROM LATERAL generate_series(1, 3) g",
        "SELECT * FROM orders o, LATERAL jsonb_array_elements(o.items) e",
        "SELECT * FROM orders o CROSS JOIN LATERAL jsonb_each_text(o.items) WITH ORDINALITY e",
        "SELECT * FROM unnest(ARRAY[1, 2]) u",
    ] {
        prepare(sql, &FakeStore::default()).unwrap_or_else(|error| panic!("{sql}: {error:?}"));
    }
}

#[test]
fn explain_with_the_british_analyse_spelling_is_refused() {
    for sql in [
        "EXPLAIN (ANALYSE) SELECT * FROM orders",
        "EXPLAIN (ANALYSE true) SELECT * FROM orders",
        "EXPLAIN (FORMAT JSON, ANALYSE on) SELECT * FROM orders",
    ] {
        assert_eq!(refusal(sql), "explain_analyze", "{sql}");
    }
    assert_eq!(refusal("EXPLAIN ANALYSE SELECT 1"), "parse_error");
    for sql in [
        "EXPLAIN (ANALYSE false) SELECT * FROM orders",
        "EXPLAIN (ANALYSE off) SELECT * FROM orders",
    ] {
        prepare(sql, &FakeStore::default()).unwrap_or_else(|error| panic!("{sql}: {error:?}"));
    }
}

#[test]
fn backup_maintenance_and_query_running_functions_are_refused() {
    for (sql, function) in [
        (
            "SELECT pg_copy_logical_replication_slot('a', 'b')",
            "pg_copy_logical_replication_slot",
        ),
        (
            "SELECT pg_copy_physical_replication_slot('a', 'b')",
            "pg_copy_physical_replication_slot",
        ),
        ("SELECT pg_backup_start('x')", "pg_backup_start"),
        ("SELECT * FROM pg_backup_stop()", "pg_backup_stop"),
        ("SELECT pg_start_backup('x')", "pg_start_backup"),
        ("SELECT pg_stop_backup()", "pg_stop_backup"),
        (
            "SELECT public.pg_stat_statements_reset()",
            "pg_stat_statements_reset",
        ),
        (
            "SELECT brin_summarize_new_values('orders_brin')",
            "brin_summarize_new_values",
        ),
        (
            "SELECT brin_summarize_range('orders_brin', 0)",
            "brin_summarize_range",
        ),
        (
            "SELECT brin_desummarize_range('orders_brin', 0)",
            "brin_desummarize_range",
        ),
        (
            "SELECT gin_clean_pending_list('orders_gin')",
            "gin_clean_pending_list",
        ),
        (
            "SELECT * FROM ts_stat('SELECT to_tsvector(note) FROM orders')",
            "ts_stat",
        ),
        (
            "SELECT ts_rewrite('a'::tsquery, 'SELECT t, s FROM aliases')",
            "ts_rewrite",
        ),
        (
            "SELECT pg_background_launch('DELETE FROM orders')",
            "pg_background_launch",
        ),
    ] {
        let error = prepare(sql, &FakeStore::default()).expect_err(sql);
        assert_eq!(error.code, "function_not_allowed", "{sql}");
        assert!(error.message.contains(function), "{sql}: {}", error.message);
    }
}

#[test]
fn statistics_views_are_refused_in_any_schema() {
    for sql in [
        "SELECT query FROM public.pg_stat_statements",
        "SELECT query FROM extensions.pg_stat_statements",
        "SELECT * FROM monitoring.pg_stat_activity_copy",
        "SELECT * FROM mydb.public.pg_statistic",
        "SELECT 1 UNION SELECT calls FROM public.pg_stat_statements",
    ] {
        assert_eq!(refusal(sql), "relation_not_allowed", "{sql}");
    }
    let code = refusal("TABLE extensions.pg_stat_statements");
    assert!(
        matches!(code.as_str(), "relation_not_allowed" | "parse_error"),
        "{code}"
    );
}

fn lineage(sql: &str) -> Option<Vec<bool>> {
    prepare(sql, &FakeStore::default())
        .unwrap_or_else(|error| panic!("{sql}: {error:?}"))
        .derived_lineage
}

#[test]
fn an_output_computed_from_a_personal_column_carries_its_lineage() {
    assert_eq!(
        lineage("SELECT split_part(email, '@', 1) FROM customers"),
        Some(vec![true])
    );
    assert_eq!(
        lineage(
            "SELECT upper(nom) AS n, amount * 2 FROM orders o JOIN customers c ON c.id = o.customer_id"
        ),
        Some(vec![true, false])
    );
}

#[test]
fn outputs_without_a_personal_source_carry_no_lineage() {
    assert_eq!(
        lineage("SELECT count(*), status FROM orders"),
        Some(vec![false, false])
    );
}

#[test]
fn lineage_follows_aliases_through_ctes_and_subqueries() {
    assert_eq!(
        lineage("WITH x AS (SELECT email AS e FROM customers) SELECT left(e, 5) FROM x"),
        Some(vec![true])
    );
    assert_eq!(
        lineage(
            "WITH x AS (SELECT email AS e FROM customers), y AS (SELECT e AS f FROM x) SELECT upper(f), 1 FROM y"
        ),
        Some(vec![true, false])
    );
    assert_eq!(
        lineage("WITH x (e) AS (SELECT email FROM customers) SELECT left(e, 5) FROM x"),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT left(s.d, 5) FROM (SELECT id, email FROM customers) AS s (i, d)"),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT left(s.i::text, 5) FROM (SELECT id, email FROM customers) AS s (i, d)"),
        Some(vec![false])
    );
    assert_eq!(
        lineage(
            "SELECT left(split_part, 5) FROM (SELECT split_part(email, '@', 1) FROM customers) s"
        ),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT to_jsonb(s) FROM (SELECT left(email, 9) AS e FROM customers) s"),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT left(a, 5) FROM customers, unnest(ARRAY[email, nom]) AS u (a)"),
        Some(vec![true])
    );
}

#[test]
fn every_branch_of_a_set_operation_contributes_lineage() {
    assert_eq!(
        lineage("SELECT 'x' UNION ALL SELECT split_part(email, '@', 1) FROM customers"),
        Some(vec![true])
    );
}

#[test]
fn lineage_is_unavailable_when_a_wildcard_hides_the_output_positions() {
    assert_eq!(lineage("SELECT * FROM customers"), None);
    assert_eq!(lineage("SELECT c.*, upper(nom) FROM customers c"), None);
    assert_eq!(lineage("EXPLAIN SELECT email FROM customers"), None);
}

#[test]
fn lineage_starts_from_personal_json_keys_and_plural_column_names() {
    assert_eq!(
        lineage("SELECT split_part(profile->'contact'->>'mail', '@', 1) FROM customers"),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT split_part(profile #>> '{contact,mail}', '@', 1) FROM customers"),
        Some(vec![true])
    );
    assert_eq!(
        lineage(
            "SELECT split_part(jsonb_extract_path_text(profile, 'contact', 'mail'), '@', 1) FROM customers"
        ),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT split_part(unnest(emails_secondaires), '@', 1) FROM customers"),
        Some(vec![true])
    );
    assert_eq!(
        lineage("SELECT profile->>'plan', upper(status) FROM customers"),
        Some(vec![false, false])
    );
}
