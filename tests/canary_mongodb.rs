mod common;

use common::mongo::{DATABASE, MONGO, MongoHarness, WILDCARD_POLICY, WRITER, filter};
use common::{assert_no_canary, list_connections, text_of};
use rmcp::model::CallToolResult;
use scarif_database_mcp::contexts::querying::presentation::FindArgs;
use serde_json::{Value, json};

const COLLECTIONS: [&str; 6] = [
    "customers",
    "orders",
    "products",
    "customer_contacts",
    "events",
    "attributes",
];

struct Transcript {
    responses: usize,
}

impl Transcript {
    fn record(&mut self, result: &CallToolResult) -> Value {
        let text = text_of(result);
        assert_no_canary(&text);
        self.responses += 1;
        serde_json::from_str(&text).unwrap_or(Value::Null)
    }

    fn reached(&mut self, result: &CallToolResult) -> Value {
        let body = self.record(result);
        assert!(body.get("error").is_none(), "{body}");
        assert!(body["document_count"].as_u64().unwrap_or(0) > 0, "{body}");
        body
    }
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn une_enquete_d_agent_sur_mongodb_ne_recoit_jamais_une_valeur_reelle_semee() {
        let harness = MongoHarness::new();
        let mut transcript = Transcript { responses: 0 };

        let connections = transcript.record(&list_connections(&harness.state).await);
        assert_eq!(connections["connections"][0]["name"], MONGO);
        assert_eq!(connections["connections"][0]["engine"], "mongodb");
        transcript.record(&harness.list_databases().await);
        transcript.record(&harness.list_tables().await);
        for pattern in ["mail", "kervadec", "name", "phone"] {
            transcript.record(&harness.search(pattern).await);
        }

        for collection in COLLECTIONS {
            transcript.record(&harness.describe(collection).await);
            let all = transcript.record(
                &harness
                    .find(
                        collection,
                        FindArgs {
                            sort: Some(json!({"_id": 1})),
                            ..FindArgs::default()
                        },
                    )
                    .await,
            );
            assert!(
                all["document_count"].as_u64().unwrap() > 0,
                "{collection}: {all}"
            );
            transcript.reached(
                &harness
                    .aggregate(collection, json!([{"$match": {}}, {"$sort": {"_id": 1}}]))
                    .await,
            );
            transcript.reached(
                &harness
                    .find(
                        collection,
                        FindArgs {
                            cell_offset: Some(5),
                            max_bytes: Some(1_024),
                            ..FindArgs::default()
                        },
                    )
                    .await,
            );
        }

        let customer = transcript.record(
            &harness
                .find(
                    "customers",
                    filter(json!({"_id": {"$oid": "65a000000000000000000001"}})),
                )
                .await,
        );
        let email = customer["documents"][0]["contact"]["email"]
            .as_str()
            .unwrap()
            .to_string();
        let last_name = customer["documents"][0]["lastName"]
            .as_str()
            .unwrap()
            .to_string();

        let orders = transcript.record(
            &harness
                .find("orders", filter(json!({"customer.email": email})))
                .await,
        );
        assert_eq!(orders["translated_literals"], 1);
        assert_eq!(orders["document_count"], 2);
        transcript.record(
            &harness
                .find(
                    "customers",
                    filter(json!({"lastName": {"$in": [last_name]}})),
                )
                .await,
        );
        transcript.record(
            &harness
                .find(
                    "customers",
                    filter(json!({"contact.email": {"$regex": format!("^{email}")}})),
                )
                .await,
        );

        for pipeline in [
            json!([{"$project": {"who": {"$concat": ["$firstName", " ", "$lastName"]}, "mail": "$contact.email", "status": 1}}]),
            json!([{"$project": {"n": "$telephone", "card": "$iban", "born": "$birthDate"}}]),
            json!([{"$project": {"whole": "$$ROOT"}}]),
            json!([{"$project": {"pairs": {"$objectToArray": "$contact"}, "keys": {"$objectToArray": "$preferences"}}}]),
            json!([{"$replaceRoot": {"newRoot": "$contact"}}]),
            json!([{"$group": {"_id": null, "names": {"$push": "$lastName"}, "firsts": {"$addToSet": "$firstName"}, "mails": {"$push": "$contact.email"}}}]),
            json!([{"$group": {"_id": "$lastName", "n": {"$sum": 1}}}]),
            json!([{"$lookup": {"from": "orders", "localField": "_id", "foreignField": "customer._id", "as": "bought"}}]),
            json!([{"$unwind": "$contact.phones"}, {"$project": {"p": "$contact.phones", "up": {"$toUpper": "$lastName"}}}]),
            json!([{"$facet": {"all": [{"$project": {"note": 1, "iban": 1}}]}}]),
            json!([{"$project": {"s": {"$substrCP": ["$contact.email", 0, 12]}, "t": {"$toString": "$telephone"}}}]),
        ] {
            transcript.reached(&harness.aggregate("customers", pipeline).await);
        }
        transcript.reached(
            &harness
                .aggregate(
                    "orders",
                    json!([{"$group": {"_id": "$status", "holders": {"$push": "$holder"}, "items": {"$push": "$items.product"}}}]),
                )
                .await,
        );
        transcript.reached(
            &harness
                .find(
                    "customers",
                    FindArgs {
                        projection: Some(json!({"email": "$contact.email", "who": "$lastName"})),
                        ..FindArgs::default()
                    },
                )
                .await,
        );

        transcript.reached(
            &harness
                .aggregate(
                    "customers",
                    json!([{"$replaceWith": {"$arrayToObject": [[{"k": "$contact.email", "v": {"$ifNull": ["$telephone", 0]}}]]}}]),
                )
                .await,
        );
        transcript.reached(
            &harness
                .aggregate(
                    "customers",
                    json!([{"$replaceWith": {"$arrayToObject": [[{"k": "$contact.email", "v": {"name": "$lastName", "first": "$firstName"}}]]}}]),
                )
                .await,
        );
        for key in [
            json!({"$concat": ["$firstName", ".", "$lastName"]}),
            json!({"$concat": ["$firstName", "_", "$lastName"]}),
        ] {
            let value = json!({"$ifNull": ["$telephone", 0]});
            transcript.reached(
                &harness
                    .aggregate(
                        "customers",
                        json!([{"$replaceWith": {"$arrayToObject": [[{"k": key, "v": value}]]}}]),
                    )
                    .await,
            );
            transcript.reached(
                &harness
                    .aggregate(
                        "customers",
                        json!([{"$project": {"m": {"$arrayToObject": [[{"k": key, "v": value}]]}}}]),
                    )
                    .await,
            );
        }
        let policed =
            MongoHarness::with_policy("[mongodb.clear]\n\"shop.orders\" = [\"status\"]\n");
        for pipeline in [
            json!([{"$project": {"status": 1, "Status": "$holder"}}]),
            json!([{"$project": {"Status": "$holder", "status": 1}}]),
        ] {
            transcript.reached(&policed.aggregate("orders", pipeline).await);
        }
        for field in ["$alias", "$badge"] {
            let error = transcript.record(
                &harness
                    .aggregate(
                        "customers",
                        json!([
                            {"$match": {"_id": {"$oid": "65a000000000000000000003"}}},
                            {"$project": {"n": {"$toInt": field}}}
                        ]),
                    )
                    .await,
            );
            assert_eq!(error["error"]["code"], "mongodb_error", "{error}");
        }

        for echoed in [
            json!({"$convert": {"input": 1, "to": "$lastName"}}),
            json!({"$dateTrunc": {"date": "$birthDate", "unit": "$lastName"}}),
            json!({"$dateTrunc": {"date": "$birthDate", "unit": "week", "startOfWeek": "$firstName"}}),
            json!({"$dateToString": {"date": "$birthDate", "timezone": "$lastName"}}),
            json!({"$regexMatch": {"input": "x", "regex": "a", "options": "$firstName"}}),
        ] {
            let refused = transcript.record(
                &harness
                    .aggregate("customers", json!([{"$project": {"n": echoed}}]))
                    .await,
            );
            assert_eq!(
                refused["error"]["code"], "mongo_argument_not_constant",
                "{refused}"
            );
        }
        let scaled = transcript.reached(
            &harness
                .aggregate(
                    "customers",
                    json!([{"$project": {"t": {"$divide": ["$telephone", 100]}, "d": {"$divide": [{"$toDecimal": "$telephone"}, 100]}}}]),
                )
                .await,
        );
        for digits in ["336412783", "337581492"] {
            assert!(!scaled.to_string().contains(digits), "{scaled}");
        }

        for failing in [
            json!([{"$project": {"n": {"$toInt": "$lastName"}}}]),
            json!([{"$project": {"d": {"$toDate": "$contact.email"}}}]),
            json!([{"$project": {"n": {"$convert": {"input": "$iban", "to": "int"}}}}]),
            json!([{"$project": {"n": {"$toDouble": "$note"}}}]),
        ] {
            let error = transcript.record(&harness.aggregate("customers", failing).await);
            assert_eq!(error["error"]["code"], "mongodb_error", "{error}");
        }
        let refused = transcript.record(
            &harness
                .aggregate("customers", json!([{"$match": {}}, {"$out": "copy"}]))
                .await,
        );
        assert_eq!(refused["error"]["category"], "refused_read_only");
        transcript.record(&harness.query("SELECT * FROM customers").await);
        transcript.record(&harness.explain("SELECT * FROM customers").await);
        transcript.record(&harness.describe("ghosts").await);

        let writer = MongoHarness::as_user(WRITER);
        transcript.record(
            &writer
                .aggregate("customers", json!([{"$merge": {"into": "copy"}}]))
                .await,
        );
        transcript.record(
            &writer
                .find(
                    "customers",
                    FindArgs {
                        database: Some(DATABASE.into()),
                        ..FindArgs::default()
                    },
                )
                .await,
        );

        assert!(transcript.responses >= 50, "{}", transcript.responses);
    }

    #[tokio::test]
    #[ignore]
    async fn un_joker_en_clair_ne_libere_ni_un_chemin_personnel_ni_un_enfant_de_parent_souille() {
        let harness = MongoHarness::with_policy(WILDCARD_POLICY);
        let mut transcript = Transcript { responses: 0 };

        let color =
            transcript.reached(&harness.find("attributes", filter(json!({"_id": 1}))).await);
        assert_eq!(
            color["documents"][0]["select_options"]["red"]["label"], "Garance Bleue",
            "{color}"
        );
        transcript.record(&harness.describe("attributes").await);

        for derived in [
            json!({"$project": {"select_options": {"$ifNull": ["$select_options", {}]}}}),
            json!({"$addFields": {"select_options": {"$mergeObjects": ["$select_options"]}}}),
            json!({"$replaceRoot": {"newRoot": {"$mergeObjects": ["$$ROOT", {}]}}}),
            json!({"$project": {"whole": "$$ROOT"}}),
        ] {
            let tainted = transcript.reached(
                &harness
                    .aggregate("attributes", json!([{"$match": {"_id": 2}}, derived]))
                    .await,
            );
            assert!(
                tainted["pseudonymized"]
                    .as_object()
                    .is_some_and(|paths| paths.values().any(|flag| flag["why"] == "derived")),
                "{tainted}"
            );
        }
        let top_level =
            MongoHarness::with_policy("[mongodb.clear]\n\"shop.attributes\" = [\"*\"]\n");
        let twin = json!({"$lookup": {"from": "attributes", "localField": "_id", "foreignField": "_id", "as": "twin"}});
        for projected in [
            json!({"$project": {"copy": {"$ifNull": ["$twin", 0]}}}),
            json!({"$project": {"twin": 1}}),
        ] {
            transcript.reached(
                &top_level
                    .aggregate("attributes", json!([twin.clone(), projected]))
                    .await,
            );
        }
        assert_eq!(transcript.responses, 8);
    }
}
