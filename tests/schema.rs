mod common;

use common::{
    CONNECTION, DescribeTableArgs, Harness, ListTablesArgs, SearchSchemaArgs, assert_no_canary,
    body, describe_table, is_error, list_tables, search_schema, text_of,
};
use rmcp::model::CallToolResult;
use serde_json::Value;

struct Response {
    error: bool,
    text: String,
    body: Value,
}

fn inspect(result: CallToolResult) -> Response {
    let text = text_of(&result);
    assert_no_canary(&text);
    Response {
        error: is_error(&result),
        body: body(&result),
        text,
    }
}

async fn search(harness: &Harness, pattern: &str) -> Response {
    inspect(
        search_schema(
            &harness.state,
            SearchSchemaArgs {
                connection: CONNECTION.to_string(),
                pattern: pattern.to_string(),
            },
        )
        .await,
    )
}

async fn describe(harness: &Harness, table: &str) -> Response {
    inspect(
        describe_table(
            &harness.state,
            DescribeTableArgs {
                connection: CONNECTION.to_string(),
                table: table.to_string(),
            },
        )
        .await,
    )
}

async fn list(harness: &Harness, args: ListTablesArgs) -> Response {
    inspect(list_tables(&harness.state, args).await)
}

fn list_args() -> ListTablesArgs {
    ListTablesArgs {
        connection: CONNECTION.to_string(),
        schema: None,
        name_like: None,
        limit: None,
        offset: None,
    }
}

fn column<'a>(described: &'a Value, name: &str) -> &'a Value {
    described["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .find(|column| column["name"] == name)
        .unwrap_or_else(|| panic!("column {name} missing in {described}"))
}

fn names(listed: &Value) -> Vec<String> {
    listed["tables"]
        .as_array()
        .expect("tables array")
        .iter()
        .map(|table| table["name"].as_str().expect("name").to_string())
        .collect()
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn la_recherche_de_schema_trouve_les_colonnes_par_sous_chaine_sans_casse() {
        let harness = Harness::new();
        let response = search(&harness, "MAIL").await;
        assert!(!response.error, "{}", response.text);
        let matches = response.body["matches"].as_array().expect("matches");
        let found: Vec<(String, String, String)> = matches
            .iter()
            .map(|item| {
                (
                    item["schema"].as_str().unwrap_or_default().to_string(),
                    item["table"].as_str().unwrap_or_default().to_string(),
                    item["column"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        assert!(found.contains(&("public".into(), "customers".into(), "email".into())));
        assert!(found.contains(&(
            "public".into(),
            "customers".into(),
            "emails_secondaires".into()
        )));
        let secondary = matches
            .iter()
            .find(|item| item["column"] == "emails_secondaires")
            .unwrap();
        assert_eq!(secondary["type"], "text[]");
        assert!(response.body.get("truncated").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn la_recherche_de_schema_trouve_une_table_par_son_nom_sans_colonne() {
        let harness = Harness::new();
        let response = search(&harness, "big_num").await;
        assert!(!response.error, "{}", response.text);
        let matches = response.body["matches"].as_array().expect("matches");
        assert!(
            matches
                .iter()
                .any(|item| item["table"] == "big_numbers" && item.get("column").is_none()),
            "{}",
            response.text
        );
    }

    #[tokio::test]
    #[ignore]
    async fn la_recherche_de_schema_traite_guillemets_et_jokers_comme_du_texte() {
        let harness = Harness::new();
        for pattern in ["it's", "%", "\\", "_x'); SELECT 1; --"] {
            let response = search(&harness, pattern).await;
            assert!(!response.error, "{pattern}: {}", response.text);
            assert_eq!(response.body["matches"], Value::Array(vec![]), "{pattern}");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_liste_ses_cles_etrangeres_sortantes() {
        let harness = Harness::new();
        let response = describe(&harness, "orders").await;
        assert!(!response.error, "{}", response.text);
        let foreign_keys = response.body["foreign_keys"].as_array().expect("fks");
        assert!(
            foreign_keys
                .iter()
                .any(|fk| fk["references"] == "public.customers(id)"
                    && fk["columns"] == serde_json::json!(["customer_id"])),
            "{}",
            response.text
        );
        assert_eq!(response.body["primary_key"], serde_json::json!(["id"]));
        let indexes = response.body["indexes"].as_array().expect("indexes");
        assert!(
            indexes
                .iter()
                .any(|index| index["name"] == "orders_customer_id_idx"
                    && index["columns"] == serde_json::json!(["customer_id"])
                    && index["unique"] == false),
            "{}",
            response.text
        );
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_liste_les_references_entrantes() {
        let harness = Harness::new();
        let response = describe(&harness, "public.customers").await;
        assert!(!response.error, "{}", response.text);
        let referenced_by = response.body["referenced_by"].as_array().expect("refs");
        for table in ["public.orders", "public.notes"] {
            assert!(
                referenced_by.iter().any(|item| item["table"] == table
                    && item["columns"] == serde_json::json!(["customer_id"])),
                "{table} missing in {}",
                response.text
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_annonce_comment_chaque_colonne_sera_pseudonymisee() {
        let harness = Harness::new();
        let response = describe(&harness, "customers").await;
        assert!(!response.error, "{}", response.text);
        let described = &response.body;
        assert_eq!(column(described, "email")["pseudonymized"], "email");
        assert_eq!(column(described, "nom")["pseudonymized"], "last_name");
        assert_eq!(column(described, "prenom")["pseudonymized"], "first_name");
        assert_eq!(column(described, "telephone")["pseudonymized"], "phone");
        assert_eq!(column(described, "ville")["pseudonymized"], "city");
        assert_eq!(
            column(described, "profile")["pseudonymized"],
            "detected_per_value"
        );
        for clear in ["id", "created_at"] {
            assert!(
                column(described, clear).get("pseudonymized").is_none(),
                "{clear} in {}",
                response.text
            );
        }
        let names: Vec<&str> = described["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|column| column["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.first(), Some(&"id"));
        assert_eq!(names.last(), Some(&"created_at"));
        assert_eq!(column(described, "email")["not_null"], true);
        assert!(column(described, "telephone").get("not_null").is_none());
        assert_eq!(column(described, "created_at")["type"], "timestamptz");
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_pseudonymise_les_donnees_des_commentaires() {
        let harness = Harness::new();
        let response = describe(&harness, "customers").await;
        assert!(!response.error, "{}", response.text);
        let comment = column(&response.body, "email")["comment"]
            .as_str()
            .expect("email comment");
        assert!(comment.starts_with("Primary contact"), "{comment}");
        assert!(!comment.contains("canary-mail"), "{comment}");
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_omet_les_valeurs_par_defaut() {
        let harness = Harness::new();
        let response = describe(&harness, "customers").await;
        assert!(!response.error, "{}", response.text);
        assert!(!response.text.contains("2026-01-15"), "{}", response.text);
        assert!(!response.text.to_lowercase().contains("default"));
    }

    #[tokio::test]
    #[ignore]
    async fn la_description_d_une_table_inconnue_est_refusee_avec_des_noms_proches() {
        let harness = Harness::new();
        let response = describe(&harness, "nope").await;
        assert!(response.error, "{}", response.text);
        assert_eq!(response.body["error"]["code"], "unknown_table");
        assert_eq!(response.body["error"]["category"], "unknown_table");

        let response = describe(&harness, "order").await;
        assert!(response.error, "{}", response.text);
        assert_eq!(response.body["error"]["code"], "unknown_table");
        let hint = response.body["error"]["hint"].as_str().expect("hint");
        assert!(hint.contains("public.orders"), "{hint}");

        let response = describe(&harness, "elsewhere.customers").await;
        assert!(response.error, "{}", response.text);
        assert_eq!(response.body["error"]["code"], "unknown_table");
    }

    #[tokio::test]
    #[ignore]
    async fn la_liste_des_tables_renvoie_des_entrees_compactes_pour_le_jeu_de_donnees() {
        let harness = Harness::new();
        let response = list(&harness, list_args()).await;
        assert!(!response.error, "{}", response.text);
        assert!(response.text.len() < 4096, "{} bytes", response.text.len());
        let listed = names(&response.body);
        for expected in ["big_numbers", "customers", "notes", "orders"] {
            assert!(listed.contains(&expected.to_string()), "{listed:?}");
        }
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);
        for table in response.body["tables"].as_array().unwrap() {
            assert_eq!(table["schema"], "public");
            assert_eq!(table["kind"], "table");
            assert!(table.get("estimated_rows").is_some(), "{table}");
            assert!(table["columns"].as_u64().unwrap() > 0, "{table}");
        }
        let customers = response.body["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["name"] == "customers")
            .unwrap();
        assert_eq!(customers["columns"], 15);
        assert!(response.body.get("next_offset").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn la_liste_des_tables_se_filtre_par_nom_et_se_pagine() {
        let harness = Harness::new();
        let response = list(
            &harness,
            ListTablesArgs {
                name_like: Some("NUM".into()),
                ..list_args()
            },
        )
        .await;
        assert!(!response.error, "{}", response.text);
        assert_eq!(names(&response.body), vec!["big_numbers".to_string()]);

        let first = list(
            &harness,
            ListTablesArgs {
                limit: Some(2),
                ..list_args()
            },
        )
        .await;
        assert_eq!(names(&first.body), vec!["big_numbers", "customers"]);
        assert_eq!(first.body["next_offset"], 2);
        let second = list(
            &harness,
            ListTablesArgs {
                limit: Some(2),
                offset: Some(2),
                ..list_args()
            },
        )
        .await;
        assert_eq!(names(&second.body), vec!["notes", "orders"]);

        let other = list(
            &harness,
            ListTablesArgs {
                schema: Some("elsewhere".into()),
                ..list_args()
            },
        )
        .await;
        assert!(!other.error, "{}", other.text);
        assert_eq!(names(&other.body), Vec::<String>::new());
    }
}
