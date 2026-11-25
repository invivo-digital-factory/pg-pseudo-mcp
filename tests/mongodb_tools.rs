mod common;

use common::mongo::{DATABASE, MongoHarness, WRITER, filter};
use common::{assert_no_canary, body, is_error, text_of};
use scarif_database_mcp::contexts::querying::presentation::{AggregateArgs, FindArgs};
use serde_json::{Value, json};

const YLWENNA: &str = "65a000000000000000000001";

fn checked(result: &rmcp::model::CallToolResult) -> Value {
    let text = text_of(result);
    assert_no_canary(&text);
    assert!(!is_error(result), "{text}");
    body(result)
}

fn refused(result: &rmcp::model::CallToolResult) -> Value {
    let text = text_of(result);
    assert_no_canary(&text);
    assert!(is_error(result), "{text}");
    body(result)["error"].clone()
}

fn by_id(id: &str) -> FindArgs {
    filter(json!({"_id": {"$oid": id}}))
}

async fn ylwenna(harness: &MongoHarness) -> Value {
    let answer = checked(&harness.find("customers", by_id(YLWENNA)).await);
    answer["documents"][0].clone()
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn ae5_un_client_revient_pseudonymise_champ_par_champ() {
        let harness = MongoHarness::new();
        let answer = checked(&harness.find("customers", by_id(YLWENNA)).await);
        let document = &answer["documents"][0];
        assert_eq!(document["_id"], json!({"$oid": YLWENNA}));
        assert!(document["birthDate"]["$date"].is_string(), "{document}");
        for path in ["firstName", "lastName", "contact.email", "contact.phones"] {
            assert!(
                answer["pseudonymized"].get(path).is_some(),
                "{path} not flagged in {answer}"
            );
        }
        assert_eq!(answer["pseudonymized"]["contact.email"]["kind"], "email");
        assert_eq!(answer["pseudonymized"]["telephone"]["kind"], "phone");
        assert!(answer["pseudonymized"].get("_id").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn ae6_le_pseudonyme_d_un_email_retrouve_les_commandes_du_client() {
        let harness = MongoHarness::new();
        let email = ylwenna(&harness).await["contact"]["email"]
            .as_str()
            .unwrap()
            .to_string();
        let answer = checked(
            &harness
                .find(
                    "orders",
                    FindArgs {
                        filter: Some(json!({"customer.email": email})),
                        sort: Some(json!({"_id": 1})),
                        ..FindArgs::default()
                    },
                )
                .await,
        );
        assert_eq!(answer["translated_literals"], 1);
        let ids: Vec<&Value> = answer["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|document| &document["_id"])
            .collect();
        assert_eq!(ids, [&json!(10), &json!(11)]);
    }

    #[tokio::test]
    #[ignore]
    async fn ae7_un_pseudonyme_dans_une_regex_reste_tel_quel_avec_un_avertissement() {
        let harness = MongoHarness::new();
        let email = ylwenna(&harness).await["contact"]["email"]
            .as_str()
            .unwrap()
            .to_string();
        let answer = checked(
            &harness
                .find(
                    "customers",
                    filter(json!({"contact.email": {"$regex": format!("^{email}")}})),
                )
                .await,
        );
        assert_eq!(answer["document_count"], 0);
        let codes: Vec<&str> = answer["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|warning| warning["code"].as_str())
            .collect();
        assert!(codes.contains(&"pseudonym_not_translated"), "{answer}");
    }

    #[tokio::test]
    #[ignore]
    async fn ae8_un_champ_calcule_est_teinte_un_renomme_garde_sa_source_un_champ_clair_reste_clair()
    {
        let harness =
            MongoHarness::with_policy("[mongodb.clear]\n\"shop.orders\" = [\"status\"]\n");
        let answer = checked(
            &harness
                .aggregate(
                    "orders",
                    json!([
                        {"$sort": {"_id": 1}},
                        {"$project": {
                            "who": {"$concat": ["$holder", " ", "$status"]},
                            "mail": "$customer.email",
                            "status": 1
                        }}
                    ]),
                )
                .await,
        );
        let first = &answer["documents"][0];
        assert_eq!(first["status"], "paid");
        assert_eq!(answer["pseudonymized"]["who"]["why"], "derived");
        assert_eq!(answer["pseudonymized"]["mail"]["kind"], "email");
        assert!(answer["pseudonymized"].get("status").is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn un_champ_renomme_garde_le_type_de_sa_source_meme_pour_un_numero() {
        let harness = MongoHarness::new();
        let answer = checked(
            &harness
                .aggregate("customers", json!([{"$project": {"n": "$telephone"}}]))
                .await,
        );
        assert_eq!(answer["pseudonymized"]["n"]["kind"], "phone");
        assert_eq!(answer["pseudonymized"]["n"]["why"], "dictionary");
    }

    #[tokio::test]
    #[ignore]
    async fn ae9_l_erreur_de_conversion_qui_cite_le_nom_reel_est_pseudonymisee() {
        let harness = MongoHarness::new();
        let error = refused(
            &harness
                .aggregate(
                    "customers",
                    json!([{"$project": {"n": {"$toInt": "$lastName"}}}]),
                )
                .await,
        );
        assert_eq!(error["code"], "mongodb_error");
        assert!(error.get("sqlstate").is_none(), "{error}");
        assert!(error["code_name"].is_string(), "{error}");
    }

    #[tokio::test]
    #[ignore]
    async fn une_requete_refusee_par_le_serveur_ne_ferme_pas_la_session() {
        let harness = MongoHarness::new();
        for _ in 0..2 {
            refused(
                &harness
                    .aggregate(
                        "customers",
                        json!([{"$project": {"n": {"$toInt": "$lastName"}}}]),
                    )
                    .await,
            );
        }
        checked(&harness.find("customers", by_id(YLWENNA)).await);
    }

    #[tokio::test]
    #[ignore]
    async fn r15_un_texte_long_se_lit_par_tranches_et_un_tableau_long_est_coupe() {
        let harness = MongoHarness::new();
        let long: String = "abcdefghij".repeat(200);
        let pipeline = json!([
            {"$match": {"_id": {"$oid": YLWENNA}}},
            {"$project": {"_id": 0, "note": {"$literal": long}, "items": {"$range": [0, 300]}}}
        ]);
        let first = checked(&harness.aggregate("customers", pipeline.clone()).await);
        let note = first["documents"][0]["note"].as_str().unwrap();
        assert!(note.starts_with(&long[..500]), "{note}");
        assert!(note.ends_with("…[+1500 chars]"), "{note}");
        assert_eq!(
            first["documents"][0]["items"].as_array().unwrap().len(),
            100
        );
        let cuts: Vec<(&str, &str)> = first["truncated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cut| {
                (
                    cut["by"].as_str().unwrap(),
                    cut["field"].as_str().unwrap_or_default(),
                )
            })
            .collect();
        assert!(cuts.contains(&("array", "items")), "{first}");
        assert!(cuts.contains(&("cell", "note")), "{first}");

        let next = checked(
            &harness
                .aggregate_with(
                    "customers",
                    AggregateArgs {
                        pipeline: pipeline.as_array().unwrap().clone(),
                        cell_offset: Some(500),
                        ..AggregateArgs::default()
                    },
                )
                .await,
        );
        let rest = next["documents"][0]["note"].as_str().unwrap();
        assert!(rest.starts_with(&long[500..1000]), "{rest}");
    }

    #[tokio::test]
    #[ignore]
    async fn r15_le_budget_d_octets_coupe_la_reponse_avec_l_indice_de_pagination() {
        let harness = MongoHarness::new();
        let answer = checked(
            &harness
                .find(
                    "customers",
                    FindArgs {
                        max_bytes: Some(1_024),
                        ..FindArgs::default()
                    },
                )
                .await,
        );
        assert!(serde_json::to_string(&answer["documents"]).unwrap().len() <= 1_024);
        let bytes = answer["truncated"]
            .as_array()
            .unwrap()
            .iter()
            .find(|cut| cut["by"] == "bytes")
            .unwrap_or_else(|| panic!("no bytes truncation in {answer}"));
        assert!(bytes["hint"].as_str().unwrap().contains("$gt"), "{answer}");
    }

    #[tokio::test]
    #[ignore]
    async fn list_tables_liste_les_collections_et_la_vue_sans_collection_systeme() {
        let harness = MongoHarness::new();
        let answer = checked(&harness.list_tables().await);
        assert_eq!(answer["database"], DATABASE);
        let names: Vec<(&str, &str)> = answer["collections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["name"].as_str().unwrap(),
                    entry["kind"].as_str().unwrap(),
                )
            })
            .collect();
        for expected in [
            ("customers", "collection"),
            ("orders", "collection"),
            ("products", "collection"),
            ("customer_contacts", "view"),
        ] {
            assert!(names.contains(&expected), "{expected:?} in {names:?}");
        }
        assert!(names.iter().all(|(name, _)| !name.starts_with("system.")));
    }

    #[tokio::test]
    #[ignore]
    async fn describe_table_donne_chemins_types_et_pseudonymisation_sans_valeur_echantillonnee() {
        let harness = MongoHarness::new();
        let answer = checked(&harness.describe("customers").await);
        let field = |path: &str| -> Value {
            answer["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|field| field["path"] == path)
                .cloned()
                .unwrap_or_else(|| panic!("no field {path} in {answer}"))
        };
        assert_eq!(field("contact.email")["pseudonymized"], "email");
        assert_eq!(field("contact.email")["types"], json!(["string"]));
        assert_eq!(field("lastName")["pseudonymized"], "last_name");
        assert_eq!(field("birthDate")["types"], json!(["date"]));
        assert!(
            answer["indexes"].to_string().contains("contact.email"),
            "{answer}"
        );
        assert!(
            answer["fields"]
                .as_array()
                .unwrap()
                .iter()
                .all(|field| !field["path"].as_str().unwrap().contains("canary-mail")),
            "{answer}"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn describe_table_sur_une_collection_absente_rend_la_table_inconnue() {
        let harness = MongoHarness::new();
        let error = refused(&harness.describe("ghosts").await);
        assert_eq!(error["code"], "unknown_table");
    }

    #[tokio::test]
    #[ignore]
    async fn list_databases_rend_les_bases_autorisees_et_la_base_par_defaut() {
        let harness = MongoHarness::new();
        let answer = checked(&harness.list_databases().await);
        assert_eq!(answer["default"], DATABASE);
        let databases: Vec<&str> = answer["databases"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(databases.contains(&DATABASE), "{answer}");
        for system in ["admin", "local", "config"] {
            assert!(!databases.contains(&system), "{answer}");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn search_schema_trouve_les_chemins_de_champs_sans_cle_reelle() {
        let harness = MongoHarness::new();
        let answer = checked(&harness.search("mail").await);
        let columns: Vec<&str> = answer["matches"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|found| found["column"].as_str())
            .collect();
        assert!(columns.contains(&"contact.email"), "{answer}");
        assert!(columns.contains(&"customer.email"), "{answer}");
    }

    #[tokio::test]
    #[ignore]
    async fn avec_un_utilisateur_read_write_un_merge_est_refuse_par_le_garde() {
        let harness = MongoHarness::as_user(WRITER);
        let error = refused(
            &harness
                .aggregate(
                    "customers",
                    json!([{"$match": {}}, {"$merge": {"into": "stolen"}}]),
                )
                .await,
        );
        assert_eq!(error["category"], "refused_read_only");
        assert!(
            error["message"].as_str().unwrap().contains("$merge"),
            "{error}"
        );
        let after = checked(&harness.find("stolen", FindArgs::default()).await);
        assert_eq!(after["document_count"], 0);
    }

    #[tokio::test]
    #[ignore]
    async fn sans_base_par_defaut_la_base_demandee_est_utilisee() {
        let harness = MongoHarness::without_default_database();
        let error = refused(&harness.find("customers", FindArgs::default()).await);
        assert_eq!(error["code"], "mongo_database_required");
        let answer = checked(
            &harness
                .find(
                    "customers",
                    FindArgs {
                        database: Some(DATABASE.into()),
                        ..by_id(YLWENNA)
                    },
                )
                .await,
        );
        assert_eq!(answer["document_count"], 1);
    }
}
