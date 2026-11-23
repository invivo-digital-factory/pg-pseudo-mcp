use std::collections::HashMap;

use crate::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;
use crate::contexts::querying::domain::errors::Refusal;
use crate::contexts::querying::domain::repositories::PseudonymTranslator;
use crate::contexts::querying::domain::services::statement_guard::inspect;
use crate::contexts::querying::domain::types::Lineage::{self, Clean, Personal, Unknown};
use crate::contexts::querying::domain::types::{
    Guarded, PersonalNames, PseudonymLookup, Translations,
};

#[derive(Default)]
struct FakeStore {
    pseudonyms: HashMap<String, String>,
}

impl FakeStore {
    fn with(mut self, pseudonym: &str, real: &str) -> Self {
        self.pseudonyms
            .insert(pseudonym.to_string(), real.to_string());
        self
    }
}

impl PseudonymTranslator for FakeStore {
    fn lookup(&self, text: &str) -> PseudonymLookup {
        if let Some(real) = self.pseudonyms.get(text) {
            return PseudonymLookup::Known { real: real.clone() };
        }
        let domain = text.rsplit_once('@').map(|(_, domain)| domain);
        if matches!(domain, Some("example.com" | "example.net" | "example.org")) {
            PseudonymLookup::Unknown
        } else {
            PseudonymLookup::NotAPseudonym
        }
    }
}

fn prepare(sql: &str, store: &FakeStore) -> Result<Guarded, Refusal> {
    let inspected = inspect(sql)?;
    let mut translations = Translations::default();
    for literal in inspected.literals() {
        translations.record(literal, store.lookup(literal));
    }
    let rewritten = inspected.rewrite(&translations)?;
    let mut personal = PersonalNames::default();
    for name in rewritten.lineage_names() {
        if DictionaryKind.execute(&name).is_some() {
            personal.insert(&name);
        }
    }
    Ok(rewritten.finish(&personal))
}

fn refusal(sql: &str) -> String {
    let error = prepare(sql, &FakeStore::default()).expect_err(sql);
    error.code
}

fn lineage(sql: &str) -> Option<Vec<Lineage>> {
    prepare(sql, &FakeStore::default())
        .unwrap_or_else(|error| panic!("{sql}: {error:?}"))
        .derived_lineage
}

mod unit {
    use super::*;

    #[test]
    fn les_requetes_de_lecture_passent_sans_changer_de_sens() {
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
            let guarded = prepare(sql, &FakeStore::default())
                .unwrap_or_else(|error| panic!("{sql}: {error:?}"));
            assert_eq!(guarded.translated_literals, 0, "{sql}");
        }
    }

    #[test]
    fn les_ecritures_et_changements_de_session_sont_refuses_avec_la_regle() {
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
    fn un_explain_analyze_est_refuse() {
        assert_eq!(
            refusal("EXPLAIN ANALYZE SELECT * FROM orders"),
            "explain_analyze"
        );
    }

    #[test]
    fn une_cte_qui_ecrit_est_refusee() {
        assert_eq!(
            refusal("WITH d AS (DELETE FROM orders RETURNING *) SELECT * FROM d"),
            "writable_cte"
        );
    }

    #[test]
    fn select_into_et_les_verrous_de_lignes_sont_refuses() {
        assert_eq!(refusal("SELECT * INTO t FROM orders"), "select_into");
        assert_eq!(refusal("SELECT * FROM orders FOR UPDATE"), "locking_clause");
    }

    #[test]
    fn plusieurs_instructions_a_la_suite_sont_refusees() {
        assert_eq!(refusal("SELECT 1; DELETE FROM orders"), "multi_statement");
    }

    #[test]
    fn les_fonctions_serveur_et_session_sont_refusees_avec_leur_nom() {
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
            (
                "SELECT * FROM crosstab('select pg_terminate_backend(1), 1, 1') AS t(a int, b int)",
                "crosstab",
            ),
            ("SELECT * FROM crosstab3('select 1, 1, 1')", "crosstab3"),
            (
                "SELECT * FROM connectby('t', 'id', 'parent', '1', 0) AS t(id text, parent text, level int)",
                "connectby",
            ),
        ] {
            let error = prepare(sql, &FakeStore::default()).expect_err(sql);
            assert_eq!(error.code, "function_not_allowed", "{sql}");
            assert!(error.message.contains(function), "{sql}: {}", error.message);
        }
    }

    #[test]
    fn les_catalogues_qui_exposent_des_donnees_sont_refuses_avec_un_indice() {
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
    fn un_sql_illisible_est_refuse_avec_le_message_de_l_analyseur() {
        let error = prepare("SELEC id FROM", &FakeStore::default()).expect_err("parse error");
        assert_eq!(error.code, "parse_error");
        assert!(!error.message.is_empty());
    }

    #[test]
    fn un_pseudonyme_connu_est_traduit_vers_la_valeur_reelle() {
        let store = FakeStore::default().with(
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
    fn un_pseudonyme_dans_un_motif_like_est_traduit_et_echappe() {
        let store =
            FakeStore::default().with("claire.martin@example.net", "ilo_na%k@canary-mail.fr");
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
    fn un_pseudonyme_numerique_est_traduit() {
        let store = FakeStore::default().with("33639981234", "33641278395");
        let guarded = prepare(
            "SELECT id FROM customers WHERE telephone_secours = 33639981234",
            &store,
        )
        .unwrap();
        assert_eq!(guarded.translated_literals, 1);
        assert!(guarded.sql.contains("33641278395"), "{}", guarded.sql);
    }

    #[test]
    fn une_valeur_reelle_avec_des_guillemets_ne_peut_pas_alterer_la_requete() {
        let store = FakeStore::default().with("Kalin Torvo", "x'); DELETE FROM orders; -- \\ $$");
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
    fn un_litteral_inconnu_en_forme_de_pseudonyme_est_laisse_avec_un_avertissement() {
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
    fn une_valeur_reelle_en_forme_de_pseudonyme_n_est_jamais_citee_dans_un_avertissement() {
        let store = FakeStore::default().with("anne.lorin7@example.net", "qa@example.org");
        let guarded = prepare(
            "SELECT id FROM customers WHERE email = 'anne.lorin7@example.net'",
            &store,
        )
        .unwrap();
        assert_eq!(guarded.translated_literals, 1);
        assert!(guarded.warnings.is_empty(), "{:?}", guarded.warnings);
    }

    #[test]
    fn le_raccourci_table_ne_donne_pas_acces_a_un_catalogue_refuse() {
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
    fn un_explain_avec_l_option_analyze_est_refuse() {
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
    fn les_reglages_et_les_identifiants_distants_sont_refuses() {
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
    fn un_pseudonyme_hors_comparaison_n_est_pas_traduit() {
        let store = FakeStore::default().with(
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
    fn un_pseudonyme_ecrit_dans_la_projection_n_est_jamais_traduit_meme_filtre_ailleurs() {
        let store = FakeStore::default().with(
            "claire.martin@example.net",
            "ylwenna.kervadec@canary-mail.fr",
        );
        let guarded = prepare(
            "SELECT concat(o.status, ' ', 'claire.martin@example.net') AS etiquette, 'claire.martin@example.net' AS brut FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = 'claire.martin@example.net'",
            &store,
        )
        .unwrap();
        assert_eq!(guarded.translated_literals, 1);
        assert_eq!(
            guarded.sql.matches("'claire.martin@example.net'").count(),
            2,
            "{}",
            guarded.sql
        );
        assert!(
            guarded
                .sql
                .ends_with("WHERE c.email = 'ylwenna.kervadec@canary-mail.fr'"),
            "{}",
            guarded.sql
        );
    }

    #[test]
    fn un_champ_json_compare_a_un_pseudonyme_est_traduit() {
        let store =
            FakeStore::default().with("claire.martin@example.net", "o.hazvekmor@canary-mail.fr");
        let guarded = prepare(
        "SELECT id FROM customers WHERE profile->'contact'->>'mail' = 'claire.martin@example.net'",
        &store,
    )
    .unwrap();
        assert_eq!(guarded.translated_literals, 1, "{}", guarded.sql);
    }

    #[test]
    fn seules_les_vues_structurelles_d_information_schema_sont_lisibles() {
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
    fn les_pseudonymes_en_comparaison_liste_et_cast_sont_traduits() {
        let store = FakeStore::default()
            .with(
                "claire.martin@example.net",
                "ylwenna.kervadec@canary-mail.fr",
            )
            .with("paul.rivet@example.org", "odrigan.hazvekmor@canary-mail.fr");
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
    fn la_forme_explain_enveloppe_une_requete_validee() {
        let guarded = prepare("SELECT * FROM orders", &FakeStore::default()).unwrap();
        assert!(guarded.explain_sql().starts_with("EXPLAIN"));
        let error = prepare("EXPLAIN SELECT 1", &FakeStore::default()).unwrap();
        assert!(error.explain_sql().matches("EXPLAIN").count() == 1);
    }

    #[test]
    fn les_fonctions_appelees_dans_from_sont_refusees_avec_leur_nom() {
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
    fn les_fonctions_de_table_ordinaires_restent_lisibles() {
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
    fn un_explain_avec_l_orthographe_analyse_est_refuse() {
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
    fn les_fonctions_de_sauvegarde_maintenance_et_execution_sont_refusees() {
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
    fn les_vues_de_statistiques_sont_refusees_dans_tout_schema() {
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

    #[test]
    fn une_sortie_calculee_depuis_une_colonne_personnelle_herite_de_sa_filiation() {
        assert_eq!(
            lineage("SELECT split_part(email, '@', 1) FROM customers"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage(
                "SELECT upper(nom) AS n, amount * 2 FROM orders o JOIN customers c ON c.id = o.customer_id"
            ),
            Some(vec![Personal, Clean])
        );
    }

    #[test]
    fn une_sortie_sans_source_personnelle_n_a_pas_de_filiation_personnelle() {
        assert_eq!(
            lineage("SELECT count(*), status FROM orders"),
            Some(vec![Clean, Clean])
        );
    }

    #[test]
    fn la_filiation_suit_les_alias_a_travers_cte_et_sous_requetes() {
        assert_eq!(
            lineage("WITH x AS (SELECT email AS e FROM customers) SELECT left(e, 5) FROM x"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage(
                "WITH x AS (SELECT email AS e FROM customers), y AS (SELECT e AS f FROM x) SELECT upper(f), 1 FROM y"
            ),
            Some(vec![Personal, Clean])
        );
        assert_eq!(
            lineage("WITH x (e) AS (SELECT email FROM customers) SELECT left(e, 5) FROM x"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT left(s.d, 5) FROM (SELECT id, email FROM customers) AS s (i, d)"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT left(s.i::text, 5) FROM (SELECT id, email FROM customers) AS s (i, d)"),
            Some(vec![Clean])
        );
        assert_eq!(
            lineage(
                "SELECT left(split_part, 5) FROM (SELECT split_part(email, '@', 1) FROM customers) s"
            ),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT to_jsonb(s) FROM (SELECT left(email, 9) AS e FROM customers) s"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT left(a, 5) FROM customers, unnest(ARRAY[email, nom]) AS u (a)"),
            Some(vec![Personal])
        );
    }

    #[test]
    fn chaque_branche_d_une_union_contribue_a_la_filiation() {
        assert_eq!(
            lineage("SELECT 'x' UNION ALL SELECT split_part(email, '@', 1) FROM customers"),
            Some(vec![Personal])
        );
    }

    #[test]
    fn toute_position_de_sortie_a_partir_d_une_etoile_est_inconnue() {
        assert_eq!(lineage("SELECT * FROM customers"), Some(vec![Unknown]));
        assert_eq!(
            lineage("SELECT *, split_part(email, '@', 1) FROM customers"),
            Some(vec![Unknown])
        );
        assert_eq!(
            lineage("SELECT c.*, upper(nom) FROM customers c"),
            Some(vec![Unknown])
        );
        assert_eq!(
            lineage("SELECT upper(status), c.*, left(email, 5) FROM customers c"),
            Some(vec![Clean, Unknown])
        );
        assert_eq!(
            lineage("SELECT left(email, 5), * FROM customers"),
            Some(vec![Personal, Unknown])
        );
        assert_eq!(
            lineage("SELECT status FROM orders UNION ALL SELECT * FROM orders"),
            Some(vec![Personal, Unknown])
        );
    }

    #[test]
    fn pas_de_filiation_pour_une_instruction_sans_lignes_propres() {
        assert_eq!(lineage("EXPLAIN SELECT email FROM customers"), None);
    }

    #[test]
    fn une_ligne_entiere_utilisee_comme_valeur_est_de_filiation_personnelle() {
        for sql in [
            "SELECT split_part(c::text, ',', 4) FROM customers c",
            "SELECT substr(c::text, 2, 20) FROM customers c",
            "SELECT row_to_json(c) FROM customers c",
            "SELECT to_jsonb(customers) FROM customers",
            "SELECT to_jsonb(customers) FROM public.customers",
            "SELECT ROW(c.*) FROM customers c",
            "SELECT to_jsonb(c.*) FROM customers c",
            "SELECT (c).email FROM customers c",
            "SELECT upper((o).status) FROM orders o",
            "SELECT left(t::text, 30) FROM customers c JOIN orders t ON t.customer_id = c.id",
            "SELECT left(s::text, 30) FROM (SELECT * FROM customers) s",
        ] {
            assert_eq!(lineage(sql), Some(vec![Personal]), "{sql}");
        }
    }

    #[test]
    fn une_colonne_qualifiee_par_sa_relation_garde_sa_propre_filiation() {
        for sql in [
            "SELECT upper(c.status) FROM customers c",
            "SELECT upper(customers.status) FROM customers",
            "SELECT to_jsonb(s) FROM (SELECT status FROM customers) s",
        ] {
            assert_eq!(lineage(sql), Some(vec![Clean]), "{sql}");
        }
    }

    #[test]
    fn la_filiation_part_des_cles_json_personnelles_et_des_colonnes_au_pluriel() {
        assert_eq!(
            lineage("SELECT split_part(profile->'contact'->>'mail', '@', 1) FROM customers"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT split_part(profile #>> '{contact,mail}', '@', 1) FROM customers"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage(
                "SELECT split_part(jsonb_extract_path_text(profile, 'contact', 'mail'), '@', 1) FROM customers"
            ),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT split_part(unnest(emails_secondaires), '@', 1) FROM customers"),
            Some(vec![Personal])
        );
        assert_eq!(
            lineage("SELECT profile->>'plan', upper(status) FROM customers"),
            Some(vec![Clean, Clean])
        );
    }
}
