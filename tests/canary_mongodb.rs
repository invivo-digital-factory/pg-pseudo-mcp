mod common;

use common::mongo::{DATABASE, MONGO, MongoHarness, WRITER, filter};
use common::{assert_no_canary, list_connections, text_of};
use rmcp::model::CallToolResult;
use scarif_database_mcp::contexts::querying::presentation::FindArgs;
use serde_json::{Value, json};

const COLLECTIONS: [&str; 5] = [
    "customers",
    "orders",
    "products",
    "customer_contacts",
    "events",
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
            transcript.record(
                &harness
                    .aggregate(collection, json!([{"$match": {}}, {"$sort": {"_id": 1}}]))
                    .await,
            );
            transcript.record(
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
            transcript.record(&harness.aggregate("customers", pipeline).await);
        }
        transcript.record(
            &harness
                .aggregate(
                    "orders",
                    json!([{"$group": {"_id": "$status", "holders": {"$push": "$holder"}, "items": {"$push": "$items.product"}}}]),
                )
                .await,
        );
        transcript.record(
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
}
