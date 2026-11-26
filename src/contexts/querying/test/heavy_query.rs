use std::collections::HashMap;

use serde_json::{Value, json};

use crate::contexts::querying::domain::services::heavy_query::{
    collection_scans, mongo_verdict, pg_verdict, relation_rows, relation_rows_sql,
    summarize_pg_plan,
};
use crate::contexts::querying::domain::types::{
    Cell, CollectionScan, HeavyQueryThresholds, PlanSummary, QueryOutput,
};

const ORDERS_ROWS: f64 = 5_000_000.0;

fn thresholds() -> HeavyQueryThresholds {
    HeavyQueryThresholds::default()
}

fn orders_rows() -> HashMap<String, f64> {
    HashMap::from([("public.orders".to_string(), ORDERS_ROWS)])
}

fn seq_scan(plan_rows: f64, total_cost: f64, filter: Option<&str>) -> Value {
    let mut node = json!({
        "Node Type": "Seq Scan",
        "Parallel Aware": false,
        "Relation Name": "orders",
        "Schema": "public",
        "Alias": "orders",
        "Startup Cost": 0.0,
        "Total Cost": total_cost,
        "Plan Rows": plan_rows,
        "Plan Width": 40,
        "Output": ["orders.id", "orders.customer_id"]
    });
    if let Some(filter) = filter {
        node["Filter"] = json!(filter);
    }
    node
}

fn limit(plan_rows: f64, total_cost: f64, child: Value) -> Value {
    json!({
        "Node Type": "Limit",
        "Startup Cost": 0.0,
        "Total Cost": total_cost,
        "Plan Rows": plan_rows,
        "Plan Width": 40,
        "Plans": [child]
    })
}

fn plan(root: Value) -> Value {
    json!([{ "Plan": root }])
}

fn summary(root: Value) -> PlanSummary {
    summarize_pg_plan(&plan(root)).expect("a JSON plan")
}

mod postgresql {
    use super::*;

    #[test]
    fn un_parcours_complet_d_une_grande_table_filtree_sans_index_est_lourd() {
        let summary = summary(seq_scan(5.0, 89_000.0, Some("(orders.customer_id = 42)")));
        let estimate = pg_verdict(&summary, &orders_rows(), thresholds()).expect("heavy");
        assert_eq!(estimate.full_scans.len(), 1);
        assert_eq!(estimate.full_scans[0].node, "Seq Scan");
        assert_eq!(estimate.full_scans[0].relation, "public.orders");
        assert_eq!(estimate.full_scans[0].estimated_rows, 5_000_000);
        assert_eq!(estimate.total_cost, Some(89_000.0));
        assert_eq!(estimate.cost_limit, Some(100_000));
        assert_eq!(estimate.scan_limit, 1_000_000);
    }

    #[test]
    fn l_estimation_ne_contient_aucun_litteral_de_la_requete() {
        let summary = summary(seq_scan(
            5.0,
            89_000.0,
            Some("(orders.email = 'jane@corp.test')"),
        ));
        let estimate = pg_verdict(&summary, &orders_rows(), thresholds()).expect("heavy");
        let shown = serde_json::to_string(&estimate).unwrap();
        assert!(!shown.contains("jane"), "{shown}");
        assert!(!shown.contains("customer_id"), "{shown}");
    }

    #[test]
    fn une_limite_qui_arrete_un_parcours_sans_filtre_le_rend_leger() {
        let summary = summary(limit(10.0, 0.18, seq_scan(ORDERS_ROWS, 89_000.0, None)));
        assert_eq!(pg_verdict(&summary, &orders_rows(), thresholds()), None);
    }

    #[test]
    fn une_limite_derriere_un_filtre_rare_n_empeche_pas_le_parcours_complet() {
        let summary = summary(limit(
            1.0,
            89_000.0,
            seq_scan(1.0, 89_000.0, Some("(orders.status = 'rare')")),
        ));
        let estimate = pg_verdict(&summary, &orders_rows(), thresholds()).expect("heavy");
        assert_eq!(estimate.full_scans[0].estimated_rows, 5_000_000);
    }

    #[test]
    fn une_limite_au_dessus_d_un_tri_ne_protege_pas_le_parcours() {
        let sort = json!({
            "Node Type": "Sort",
            "Total Cost": 900_000.0,
            "Plan Rows": ORDERS_ROWS,
            "Plans": [seq_scan(ORDERS_ROWS, 89_000.0, None)]
        });
        let summary = summary(limit(10.0, 900_000.0, sort));
        let estimate = pg_verdict(&summary, &orders_rows(), thresholds()).expect("heavy");
        assert_eq!(estimate.full_scans[0].estimated_rows, 5_000_000);
        assert_eq!(estimate.total_cost, Some(900_000.0));
    }

    #[test]
    fn un_cout_total_au_dessus_du_seuil_suffit_sans_parcours_complet() {
        let summary = summary(json!({
            "Node Type": "Hash Join",
            "Total Cost": 250_000.5,
            "Plan Rows": 20,
            "Plans": [
                {"Node Type": "Index Scan", "Relation Name": "orders", "Schema": "public", "Total Cost": 8.4, "Plan Rows": 1},
                {"Node Type": "Hash", "Total Cost": 10.0, "Plan Rows": 1}
            ]
        }));
        let estimate = pg_verdict(&summary, &HashMap::new(), thresholds()).expect("heavy");
        assert!(estimate.full_scans.is_empty());
        assert_eq!(estimate.total_cost, Some(250_000.5));
    }

    #[test]
    fn une_lecture_par_index_peu_couteuse_n_est_pas_lourde() {
        let summary = summary(json!({
            "Node Type": "Index Scan",
            "Relation Name": "orders",
            "Schema": "public",
            "Total Cost": 8.44,
            "Plan Rows": 1
        }));
        assert_eq!(pg_verdict(&summary, &orders_rows(), thresholds()), None);
    }

    #[test]
    fn les_seuils_de_la_connexion_remplacent_les_seuils_par_defaut() {
        let summary = summary(seq_scan(10.0, 12.0, None));
        let tight = HeavyQueryThresholds::with_overrides(Some(5), Some(100_000));
        let estimate = pg_verdict(&summary, &HashMap::new(), tight).expect("heavy");
        assert_eq!(estimate.cost_limit, Some(5));
        assert!(estimate.full_scans.is_empty());
        let rows = HashMap::from([("public.orders".to_string(), 200_000.0)]);
        let estimate = pg_verdict(&summary, &rows, tight).expect("heavy");
        assert_eq!(estimate.full_scans[0].estimated_rows, 200_000);
    }

    #[test]
    fn sans_estimation_du_catalogue_les_lignes_du_plan_servent_d_estimation() {
        let summary = summary(seq_scan(2_000_000.0, 40_000.0, None));
        let never_analyzed = HashMap::from([("public.orders".to_string(), -1.0)]);
        for rows in [HashMap::new(), never_analyzed] {
            let estimate = pg_verdict(&summary, &rows, thresholds()).expect("heavy");
            assert_eq!(estimate.full_scans[0].estimated_rows, 2_000_000);
        }
    }

    #[test]
    fn un_plan_qui_n_est_pas_du_json_d_explain_n_est_pas_lu() {
        assert_eq!(summarize_pg_plan(&json!("Seq Scan on orders")), None);
        assert_eq!(summarize_pg_plan(&json!([{"Plan": {}}])), None);
    }

    #[test]
    fn le_catalogue_est_interroge_une_fois_par_relation_parcourue_avec_des_noms_echappes() {
        let summary = summary(json!({
            "Node Type": "Append",
            "Total Cost": 10.0,
            "Plan Rows": 2,
            "Plans": [
                seq_scan(1.0, 1.0, None),
                seq_scan(1.0, 1.0, None),
                {"Node Type": "Seq Scan", "Relation Name": "it's", "Schema": "public", "Total Cost": 1.0, "Plan Rows": 1},
                {"Node Type": "Seq Scan", "Relation Name": "nameless", "Total Cost": 1.0, "Plan Rows": 1}
            ]
        }));
        let sql = relation_rows_sql(&summary).expect("catalog query");
        assert_eq!(sql.matches("E'orders'").count(), 1, "{sql}");
        assert!(sql.contains("E'it''s'"), "{sql}");
        assert!(!sql.contains("nameless"), "{sql}");
        assert_eq!(relation_rows_sql(&summary_without_scans()), None);
    }

    fn summary_without_scans() -> PlanSummary {
        PlanSummary {
            total_cost: 1.0,
            scans: Vec::new(),
        }
    }

    #[test]
    fn les_lignes_estimees_du_catalogue_sont_rangees_par_nom_qualifie() {
        let output = QueryOutput {
            columns: Vec::new(),
            rows: vec![vec![
                Cell::Text("public".into()),
                Cell::Text("orders".into()),
                Cell::Float(ORDERS_ROWS),
            ]],
            more_rows: false,
            notices: Vec::new(),
        };
        assert_eq!(relation_rows(&output), orders_rows());
    }
}

mod mongodb {
    use super::*;

    fn counts(count: u64) -> HashMap<String, u64> {
        HashMap::from([("orders".to_string(), count)])
    }

    fn classic(winning_plan: Value) -> Value {
        json!({
            "queryPlanner": {
                "namespace": "shop.orders",
                "winningPlan": winning_plan,
                "rejectedPlans": [{"stage": "COLLSCAN", "filter": {"x": 1}}]
            },
            "command": {"find": "orders", "filter": {"winningPlan": {"stage": "COLLSCAN"}}}
        })
    }

    #[test]
    fn un_find_filtre_sans_index_sur_une_grande_collection_est_lourd() {
        let explain = classic(json!({
            "stage": "LIMIT",
            "limitAmount": 51,
            "inputStage": {"stage": "COLLSCAN", "filter": {"status": {"$eq": "PAID"}}, "direction": "forward"}
        }));
        let scans = collection_scans(&explain);
        assert_eq!(
            scans,
            vec![CollectionScan {
                collection: "orders".into(),
                filtered: true,
                stopped_by_limit: true
            }]
        );
        let estimate = mongo_verdict(&scans, &counts(5_000_000), thresholds()).expect("heavy");
        assert_eq!(estimate.full_scans[0].node, "COLLSCAN");
        assert_eq!(estimate.full_scans[0].relation, "orders");
        assert_eq!(estimate.full_scans[0].estimated_rows, 5_000_000);
        assert_eq!(estimate.total_cost, None);
        let shown = serde_json::to_string(&estimate).unwrap();
        assert!(!shown.contains("PAID"), "{shown}");
    }

    #[test]
    fn un_find_sans_filtre_arrete_par_sa_limite_est_leger() {
        let explain = classic(json!({
            "stage": "LIMIT",
            "limitAmount": 51,
            "inputStage": {"stage": "PROJECTION_SIMPLE", "inputStage": {"stage": "COLLSCAN", "direction": "forward"}}
        }));
        let scans = collection_scans(&explain);
        assert_eq!(scans.len(), 1);
        assert_eq!(
            mongo_verdict(&scans, &counts(5_000_000), thresholds()),
            None
        );
    }

    #[test]
    fn un_tri_sans_index_avant_la_limite_parcourt_toute_la_collection() {
        let explain = classic(json!({
            "stage": "SORT",
            "limitAmount": 51,
            "inputStage": {"stage": "COLLSCAN", "direction": "forward"}
        }));
        let scans = collection_scans(&explain);
        assert!(mongo_verdict(&scans, &counts(5_000_000), thresholds()).is_some());
    }

    #[test]
    fn une_agregation_qui_groupe_toute_la_collection_est_lourde() {
        let explain = json!({
            "stages": [
                {"$cursor": {"queryPlanner": {
                    "namespace": "shop.orders",
                    "winningPlan": {"stage": "PROJECTION_SIMPLE", "inputStage": {"stage": "COLLSCAN", "direction": "forward"}}
                }}},
                {"$group": {"_id": "$status"}},
                {"$limit": 51}
            ]
        });
        let scans = collection_scans(&explain);
        assert!(mongo_verdict(&scans, &counts(5_000_000), thresholds()).is_some());
    }

    #[test]
    fn le_plan_du_moteur_sbe_est_lu_dans_query_plan() {
        let explain = json!({
            "queryPlanner": {
                "namespace": "shop.orders",
                "winningPlan": {
                    "queryPlan": {"stage": "GROUP", "inputStage": {"stage": "COLLSCAN", "direction": "forward"}},
                    "slotBasedPlan": {"slots": "", "stages": ""}
                }
            }
        });
        let scans = collection_scans(&explain);
        assert!(mongo_verdict(&scans, &counts(5_000_000), thresholds()).is_some());
    }

    #[test]
    fn une_lecture_par_index_n_est_pas_lourde() {
        let explain = classic(json!({
            "stage": "LIMIT",
            "inputStage": {"stage": "FETCH", "inputStage": {"stage": "IXSCAN", "indexName": "status_1"}}
        }));
        assert!(collection_scans(&explain).is_empty());
    }

    #[test]
    fn une_petite_collection_ou_un_compte_inconnu_ne_bloque_pas() {
        let explain = classic(json!({"stage": "COLLSCAN", "filter": {"status": {"$eq": "PAID"}}}));
        let scans = collection_scans(&explain);
        assert_eq!(mongo_verdict(&scans, &counts(1_000), thresholds()), None);
        assert_eq!(mongo_verdict(&scans, &HashMap::new(), thresholds()), None);
    }

    #[test]
    fn le_seuil_de_la_connexion_remplace_le_seuil_par_defaut() {
        let explain = classic(json!({"stage": "COLLSCAN", "filter": {"status": {"$eq": "PAID"}}}));
        let scans = collection_scans(&explain);
        let tight = HeavyQueryThresholds::with_overrides(None, Some(500));
        let estimate = mongo_verdict(&scans, &counts(1_000), tight).expect("heavy");
        assert_eq!(estimate.scan_limit, 500);
    }
}
