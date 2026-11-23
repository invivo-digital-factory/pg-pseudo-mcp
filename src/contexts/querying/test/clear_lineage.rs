use crate::contexts::anonymization::domain::types::ColumnInfo;
use crate::contexts::anonymization::use_cases::classify_column::ClassifyColumn;
use crate::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;
use crate::contexts::querying::domain::repositories::PersonalColumnDictionary;
use crate::contexts::querying::domain::services::clear_lineage::{
    catalog_relations, clearances, is_clear_source,
};
use crate::contexts::querying::domain::services::statement_parser::parse_single;
use crate::contexts::querying::domain::types::Clearance::{ProvenClear, Unproven};
use crate::contexts::querying::domain::types::{
    CatalogColumn, Clearance, ColumnClassification, RelationCatalog,
};

struct Dictionary;

impl PersonalColumnDictionary for Dictionary {
    fn dictionary_kind(&self, column_name: &str) -> Option<String> {
        DictionaryKind
            .execute(column_name)
            .map(|kind| kind.as_str().to_string())
    }

    fn classify(&self, column_name: &str, type_name: &str) -> ColumnClassification {
        let class = ClassifyColumn.execute(&ColumnInfo {
            column_name: Some(column_name),
            type_name,
            derived: false,
        });
        ColumnClassification {
            forced_kind: class.forced_kind.map(|kind| kind.as_str().to_string()),
            clear_by_type: class.clear_by_type,
        }
    }
}

fn columns(definitions: &[(&str, &str)]) -> Vec<CatalogColumn> {
    definitions
        .iter()
        .map(|(name, type_name)| CatalogColumn {
            name: name.to_string(),
            type_name: type_name.to_string(),
        })
        .collect()
}

fn catalog() -> RelationCatalog {
    let mut catalog = RelationCatalog::default();
    let orders = columns(&[
        ("id", "int4"),
        ("customer_id", "int4"),
        ("order_ref", "uuid"),
        ("status", "enum"),
        ("payment_method", "enum"),
        ("paid", "bool"),
        ("amount", "numeric"),
        ("created_at", "timestamptz"),
        ("status_label", "text"),
    ]);
    catalog.insert("orders", orders.clone());
    catalog.insert("public.orders", orders);
    catalog.insert(
        "customers",
        columns(&[
            ("id", "int4"),
            ("nom", "text"),
            ("prenom", "text"),
            ("telephone_secours", "int8"),
            ("date_naissance", "date"),
            ("created_at", "timestamptz"),
        ]),
    );
    catalog
}

fn verdicts(sql: &str, column_count: usize) -> Vec<Clearance> {
    let statement = parse_single(sql).unwrap_or_else(|error| panic!("{sql}: {error:?}"));
    clearances(
        &statement,
        &catalog(),
        &|column| is_clear_source(&Dictionary, column),
        column_count,
    )
}

fn verdict(sql: &str) -> Clearance {
    verdicts(sql, 1)[0]
}

mod unit {
    use super::*;

    #[test]
    fn une_concatenation_de_deux_enums_et_d_un_litteral_reste_en_clair() {
        assert_eq!(
            verdict("SELECT concat(status, ' - ', payment_method) FROM orders"),
            ProvenClear
        );
    }

    #[test]
    fn un_enum_converti_concatene_a_une_date_reste_en_clair() {
        for sql in [
            "SELECT status::text || created_at::date FROM orders",
            "SELECT status::text || '/' || CAST(created_at AS date) FROM public.orders o",
            "SELECT pg_catalog.lower(o.status::text) || to_char(o.created_at, 'YYYY-MM') FROM orders AS o",
        ] {
            assert_eq!(verdict(sql), ProvenClear, "{sql}");
        }
    }

    #[test]
    fn un_comptage_converti_en_texte_reste_en_clair() {
        for sql in [
            "SELECT count(*)::text FROM orders",
            "SELECT count(DISTINCT customer_id)::text FROM orders",
            "SELECT string_agg(status::text, ',' ORDER BY created_at) FROM orders",
        ] {
            assert_eq!(verdict(sql), ProvenClear, "{sql}");
        }
    }

    #[test]
    fn un_case_sur_des_colonnes_en_clair_reste_en_clair() {
        for sql in [
            "SELECT CASE WHEN paid THEN 'yes' END FROM orders",
            "SELECT CASE WHEN amount > 100 AND NOT paid THEN upper(status::text) ELSE 'n/a' END FROM orders",
            "SELECT coalesce(nullif(status::text, 'paid'), 'none') FROM orders",
            "SELECT round(amount * 1.2, 2)::text FROM orders",
            "SELECT extract(year FROM created_at)::text FROM orders",
            "SELECT order_ref::text FROM orders",
        ] {
            assert_eq!(verdict(sql), ProvenClear, "{sql}");
        }
    }

    #[test]
    fn une_concatenation_d_un_enum_et_d_un_nom_reste_pseudonymisee() {
        assert_eq!(
            verdict(
                "SELECT concat(o.status, ' ', c.nom) FROM orders o JOIN customers c ON c.id = o.customer_id"
            ),
            Unproven
        );
    }

    #[test]
    fn un_nom_passe_en_majuscules_reste_pseudonymise() {
        assert_eq!(verdict("SELECT upper(nom) FROM customers"), Unproven);
    }

    #[test]
    fn une_colonne_texte_n_est_jamais_une_source_en_clair() {
        assert_eq!(verdict("SELECT lower(status_label) FROM orders"), Unproven);
    }

    #[test]
    fn une_colonne_du_dictionnaire_n_est_pas_une_source_en_clair_meme_typee_en_clair() {
        for sql in [
            "SELECT telephone_secours::text FROM customers",
            "SELECT date_naissance::text FROM customers",
        ] {
            assert_eq!(verdict(sql), Unproven, "{sql}");
        }
    }

    #[test]
    fn une_fonction_utilisateur_sur_un_uuid_reste_pseudonymisee() {
        for sql in [
            "SELECT get_customer_name(order_ref) FROM orders",
            "SELECT public.lower(status) FROM orders",
            "SELECT get_customer_name(order_ref)::text || status::text FROM orders",
        ] {
            assert_eq!(verdict(sql), Unproven, "{sql}");
        }
    }

    #[test]
    fn une_sous_requete_scalaire_reste_pseudonymisee() {
        for sql in [
            "SELECT (SELECT status::text FROM orders LIMIT 1) FROM customers",
            "SELECT EXISTS (SELECT 1 FROM orders) FROM customers",
            "SELECT status IN (SELECT status FROM orders) FROM orders",
        ] {
            assert_eq!(verdict(sql), Unproven, "{sql}");
        }
    }

    #[test]
    fn un_enum_transmis_par_une_cte_reste_en_clair() {
        for sql in [
            "WITH s AS (SELECT status AS etat FROM orders) SELECT etat::text FROM s",
            "WITH s(etat) AS (SELECT status FROM orders) SELECT upper(s.etat::text) FROM s",
            "SELECT d.etat FROM (SELECT status::text AS etat FROM orders) d",
        ] {
            assert_eq!(verdict(sql), ProvenClear, "{sql}");
        }
    }

    #[test]
    fn une_cte_qui_transmet_un_nom_reste_pseudonymisee() {
        assert_eq!(
            verdict("WITH s AS (SELECT nom AS etat FROM customers) SELECT etat::text FROM s"),
            Unproven
        );
    }

    #[test]
    fn une_union_dont_une_branche_est_du_texte_reste_pseudonymisee() {
        assert_eq!(
            verdict("SELECT status::text FROM orders UNION SELECT nom FROM customers"),
            Unproven
        );
        assert_eq!(
            verdict(
                "SELECT status::text FROM orders UNION ALL SELECT payment_method::text FROM orders"
            ),
            ProvenClear
        );
    }

    #[test]
    fn une_reference_ambigue_ou_introuvable_n_est_pas_prouvee() {
        for sql in [
            "SELECT id::text FROM orders JOIN customers ON true",
            "SELECT created_at::date FROM orders, customers",
            "SELECT x::text FROM nowhere",
            "SELECT missing::text FROM orders",
            "SELECT o::text FROM orders o",
            "SELECT current_user",
        ] {
            assert_eq!(verdict(sql), Unproven, "{sql}");
        }
    }

    #[test]
    fn une_etoile_ou_un_decalage_de_colonnes_ne_prouve_rien() {
        assert_eq!(
            verdicts("SELECT status::text, * FROM orders", 3),
            vec![Unproven; 3]
        );
        assert_eq!(
            verdicts("SELECT status::text, payment_method::text FROM orders", 3),
            vec![Unproven; 3]
        );
        assert_eq!(
            verdict("WITH s AS (SELECT * FROM orders) SELECT status::text FROM s"),
            Unproven
        );
    }

    #[test]
    fn chaque_colonne_de_la_projection_est_jugee_a_sa_position() {
        assert_eq!(
            verdicts(
                "SELECT status::text, c.nom, count(*) OVER (PARTITION BY o.status)::text FROM orders o JOIN customers c ON c.id = o.customer_id",
                3
            ),
            vec![ProvenClear, Unproven, ProvenClear]
        );
    }

    #[test]
    fn une_fenetre_sur_une_colonne_personnelle_n_est_pas_prouvee() {
        assert_eq!(
            verdict("SELECT count(*) OVER (PARTITION BY nom)::text FROM customers"),
            Unproven
        );
    }

    #[test]
    fn une_cte_recursive_ou_une_jointure_laterale_n_est_pas_prouvee() {
        for sql in [
            "WITH RECURSIVE s AS (SELECT status FROM orders) SELECT status::text FROM s",
            "SELECT l.s FROM orders o, LATERAL (SELECT o.status::text AS s) l",
        ] {
            assert_eq!(verdict(sql), Unproven, "{sql}");
        }
    }

    #[test]
    fn une_requete_sans_colonne_lue_reste_en_clair() {
        assert_eq!(verdict("SELECT 'Martin' || 1"), ProvenClear);
    }

    #[test]
    fn un_explain_ne_prouve_rien() {
        assert_eq!(verdict("EXPLAIN SELECT status::text FROM orders"), Unproven);
    }

    #[test]
    fn les_relations_lues_sont_celles_du_catalogue_hors_cte() {
        let statement = parse_single(
            "WITH s AS (SELECT status FROM public.orders) SELECT s.status::text FROM s JOIN customers c ON true WHERE c.id IN (SELECT customer_id FROM orders)",
        )
        .unwrap();
        assert_eq!(
            catalog_relations(&statement),
            ["customers", "orders", "public.orders"]
        );
    }
}
