mod common;

use common::{
    CONNECTION, DescribeTableArgs, Harness, QueryArgs, assert_no_canary, describe_table, is_error,
    list_connections_with_policy_warnings, query, text_of,
};
use serde_json::{Value, json};

const HINT: &str = "classify it in the column policy: clear or personal";

async fn ask(harness: &Harness, sql: &str) -> Value {
    let result = query(
        &harness.state,
        QueryArgs {
            connection: CONNECTION.to_string(),
            sql: sql.to_string(),
            ..QueryArgs::default()
        },
    )
    .await;
    let text = text_of(&result);
    assert_no_canary(&text);
    assert!(!is_error(&result), "{text}");
    serde_json::from_str(&text).unwrap()
}

async fn describe(harness: &Harness, table: &str) -> Value {
    let result = describe_table(
        &harness.state,
        DescribeTableArgs {
            connection: CONNECTION.to_string(),
            table: table.to_string(),
            database: None,
        },
    )
    .await;
    let text = text_of(&result);
    assert!(!is_error(&result), "{text}");
    serde_json::from_str(&text).unwrap()
}

fn hinted_columns(body: &Value) -> Vec<String> {
    body.get("column_policy_hints")
        .and_then(Value::as_array)
        .map(|hints| {
            hints
                .iter()
                .map(|hint| hint["column"].as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn une_colonne_declaree_en_clair_rend_sa_valeur_en_clair() {
        let baseline = ask(
            &Harness::new(),
            "SELECT product_name FROM products WHERE id = 1",
        )
        .await;
        assert_ne!(baseline["rows"][0][0], "Garance Bleue", "{baseline}");
        assert_eq!(
            baseline["pseudonymized"]["product_name"]["why"],
            "dictionary"
        );

        let harness = Harness::with_column_policy(Some("clear = [\"*.product_name\"]"), None);
        let body = ask(&harness, "SELECT product_name FROM products WHERE id = 1").await;
        assert_eq!(body["rows"][0][0], "Garance Bleue", "{body}");
        assert!(body.get("pseudonymized").is_none(), "{body}");
    }

    #[tokio::test]
    #[ignore]
    async fn une_colonne_declaree_en_clair_remplace_quand_meme_une_adresse_e_mail() {
        let harness = Harness::with_column_policy(Some("clear = [\"*.product_name\"]"), None);
        let body = ask(&harness, "SELECT product_name FROM products WHERE id = 2").await;
        let value = body["rows"][0][0].as_str().unwrap();
        assert!(value.starts_with("Garance "), "{body}");
        assert!(!value.contains("canary-mail"), "{body}");
        assert_eq!(
            body["pseudonymized"]["product_name"],
            json!({ "kind": "email", "why": "detected" })
        );
    }

    #[tokio::test]
    #[ignore]
    async fn une_colonne_declaree_personnelle_est_pseudonymisee_avec_le_type_donne() {
        let baseline = ask(
            &Harness::new(),
            "SELECT supplier_ref FROM products WHERE id = 1",
        )
        .await;
        assert_eq!(baseline["rows"][0][0], "lot-42", "{baseline}");

        let harness = Harness::with_column_policy(
            Some("[personal]\n\"products.supplier_ref\" = \"last_name\""),
            None,
        );
        let body = ask(&harness, "SELECT supplier_ref FROM products WHERE id = 1").await;
        assert_ne!(body["rows"][0][0], "lot-42", "{body}");
        assert_eq!(
            body["pseudonymized"]["supplier_ref"],
            json!({ "kind": "last_name", "why": "policy" })
        );
    }

    #[tokio::test]
    #[ignore]
    async fn le_fichier_local_l_emporte_sur_le_fichier_partage() {
        let harness = Harness::with_column_policy(
            Some("[personal]\n\"*.product_name\" = \"last_name\""),
            Some("clear = [\"*.product_name\"]"),
        );
        let body = ask(&harness, "SELECT product_name FROM products WHERE id = 1").await;
        assert_eq!(body["rows"][0][0], "Garance Bleue", "{body}");
    }

    #[tokio::test]
    #[ignore]
    async fn schema_table_colonne_l_emporte_sur_l_etoile() {
        let harness = Harness::with_column_policy(
            Some(
                "clear = [\"*.product_name\"]\n[personal]\n\"public.products.product_name\" = \"last_name\"",
            ),
            None,
        );
        let body = ask(&harness, "SELECT product_name FROM products WHERE id = 1").await;
        assert_ne!(body["rows"][0][0], "Garance Bleue", "{body}");
        assert_eq!(body["pseudonymized"]["product_name"]["why"], "policy");
    }

    #[tokio::test]
    #[ignore]
    async fn un_fichier_invalide_est_signale_et_ne_change_rien() {
        let harness = Harness::with_column_policy(
            Some(
                "clear = [\"*.product_name\"]\n[personal]\n\"*.supplier_ref\" = \"numero_fiscal\"",
            ),
            None,
        );
        let listing = list_connections_with_policy_warnings(&harness.state).await;
        let listing: Value = serde_json::from_str(&text_of(&listing)).unwrap();
        let message = listing["warnings"][0]["message"].as_str().unwrap();
        assert!(message.contains("team-columns.toml"), "{listing}");
        assert!(message.contains("numero_fiscal"), "{listing}");
        assert!(!listing["connections"].as_array().unwrap().is_empty());

        let body = ask(
            &harness,
            "SELECT product_name, supplier_ref FROM products WHERE id = 1",
        )
        .await;
        assert_ne!(body["rows"][0][0], "Garance Bleue", "{body}");
        assert_eq!(body["rows"][0][1], "lot-42", "{body}");
    }

    #[tokio::test]
    #[ignore]
    async fn une_colonne_non_classee_porte_un_indice_qui_disparait_une_fois_classee() {
        let sql = "SELECT p.product_name, o.titulaire, c.email FROM products p, orders o, customers c WHERE p.id = 1 AND o.id = 10 AND c.id = 1";
        let unclassified = ask(&Harness::new(), sql).await;
        assert_eq!(
            unclassified["column_policy_hints"],
            json!([
                { "column": "public.products.product_name", "reason": "dictionary", "hint": HINT },
                { "column": "public.orders.titulaire", "reason": "default", "hint": HINT },
            ]),
            "{unclassified}"
        );

        let harness = Harness::with_column_policy(
            Some(
                "clear = [\"*.product_name\"]\n[personal]\n\"orders.titulaire\" = \"person_name\"\n\"customers.email\" = \"email\"",
            ),
            None,
        );
        let classified = ask(&harness, sql).await;
        assert!(
            classified.get("column_policy_hints").is_none(),
            "{classified}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_suit_la_politique_et_signale_les_colonnes_a_classer() {
        let unclassified = describe(&Harness::new(), "products").await;
        assert_eq!(
            hinted_columns(&unclassified),
            vec![
                "public.products.product_name".to_string(),
                "public.products.supplier_ref".to_string()
            ],
            "{unclassified}"
        );

        let harness = Harness::with_column_policy(
            Some("clear = [\"*.product_name\"]\n[personal]\n\"products.supplier_ref\" = \"siret\""),
            None,
        );
        let classified = describe(&harness, "products").await;
        assert!(hinted_columns(&classified).is_empty(), "{classified}");
        let column = |name: &str| {
            classified["columns"]
                .as_array()
                .unwrap()
                .iter()
                .find(|column| column["name"] == name)
                .unwrap()
                .clone()
        };
        assert_eq!(column("product_name")["policy"], "clear");
        assert!(column("product_name").get("pseudonymized").is_none());
        assert_eq!(column("supplier_ref")["policy"], "personal");
        assert_eq!(column("supplier_ref")["pseudonymized"], "siret");
    }

    #[tokio::test]
    #[ignore]
    async fn une_colonne_declaree_en_clair_nourrit_la_lignee_des_expressions() {
        let sql = "SELECT concat(product_name, ' ', status) AS v FROM products WHERE id = 1";
        let baseline = ask(&Harness::new(), sql).await;
        assert_ne!(baseline["rows"][0][0], "Garance Bleue paid", "{baseline}");

        let harness = Harness::with_column_policy(Some("clear = [\"*.product_name\"]"), None);
        let body = ask(&harness, sql).await;
        assert_eq!(body["rows"][0][0], "Garance Bleue paid", "{body}");
        assert!(body.get("pseudonymized").is_none(), "{body}");

        let leaked = ask(
            &harness,
            "SELECT concat(product_name, ' ', status) AS v FROM products WHERE id = 2",
        )
        .await;
        let value = leaked["rows"][0][0].as_str().unwrap();
        assert!(!value.contains("canary-mail"), "{leaked}");
    }
}
