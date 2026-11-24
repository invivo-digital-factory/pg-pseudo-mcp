use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Map, Value, json};

use crate::contexts::querying::domain::types::DocumentNode;

pub fn render(node: DocumentNode) -> Value {
    match node {
        DocumentNode::Null => Value::Null,
        DocumentNode::Bool(flag) => Value::Bool(flag),
        DocumentNode::Int(number) => json!(number),
        DocumentNode::Double(number) => double(number),
        DocumentNode::Decimal(text) => json!({ "$numberDecimal": text }),
        DocumentNode::Text(text) => Value::String(text),
        DocumentNode::Date(text) => date(text),
        DocumentNode::ObjectId(hex) => json!({ "$oid": hex }),
        DocumentNode::Uuid { subtype, value } => uuid(subtype, value),
        DocumentNode::Timestamp(text) => timestamp(text),
        DocumentNode::MinKey => json!({ "$minKey": 1 }),
        DocumentNode::MaxKey => json!({ "$maxKey": 1 }),
        DocumentNode::Binary { len, .. } => Value::String(format!("<binary len={len}>")),
        DocumentNode::Opaque { type_name, len } => {
            Value::String(format!("<{type_name} len={len}>"))
        }
        DocumentNode::Array(items) => Value::Array(items.into_iter().map(render).collect()),
        DocumentNode::Object(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key, render(value)))
                .collect::<Map<String, Value>>(),
        ),
    }
}

fn double(number: f64) -> Value {
    if number.is_finite() {
        return json!(number);
    }
    let text = if number.is_nan() {
        "NaN"
    } else if number > 0.0 {
        "Infinity"
    } else {
        "-Infinity"
    };
    json!({ "$numberDouble": text })
}

fn date(text: String) -> Value {
    if text.parse::<i64>().is_ok() {
        json!({ "$date": { "$numberLong": text } })
    } else {
        json!({ "$date": text })
    }
}

fn uuid(subtype: u8, value: String) -> Value {
    match hex::decode(value.replace('-', "")) {
        Ok(bytes) => json!({
            "$binary": { "base64": STANDARD.encode(bytes), "subType": format!("{subtype:02x}") }
        }),
        Err(_) => Value::String(value),
    }
}

fn timestamp(text: String) -> Value {
    let parsed = text.split_once(':').and_then(|(time, increment)| {
        Some((time.parse::<u32>().ok()?, increment.parse::<u32>().ok()?))
    });
    match parsed {
        Some((time, increment)) => json!({ "$timestamp": { "t": time, "i": increment } }),
        None => Value::String(text),
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn chaque_noeud_prend_la_forme_relachee_de_l_extended_json() {
        let cases = [
            (DocumentNode::Null, json!(null)),
            (DocumentNode::Bool(true), json!(true)),
            (DocumentNode::Int(-7), json!(-7)),
            (DocumentNode::Double(0.25), json!(0.25)),
            (
                DocumentNode::Double(f64::NAN),
                json!({"$numberDouble": "NaN"}),
            ),
            (
                DocumentNode::Double(f64::NEG_INFINITY),
                json!({"$numberDouble": "-Infinity"}),
            ),
            (
                DocumentNode::Decimal("12345.6700".into()),
                json!({"$numberDecimal": "12345.6700"}),
            ),
            (DocumentNode::Text("paid".into()), json!("paid")),
            (
                DocumentNode::Date("2026-03-01T09:30:00.000Z".into()),
                json!({"$date": "2026-03-01T09:30:00.000Z"}),
            ),
            (
                DocumentNode::Date("-62167219200001".into()),
                json!({"$date": {"$numberLong": "-62167219200001"}}),
            ),
            (
                DocumentNode::ObjectId("65a000000000000000000001".into()),
                json!({"$oid": "65a000000000000000000001"}),
            ),
            (
                DocumentNode::Uuid {
                    subtype: 4,
                    value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
                },
                json!({"$binary": {"base64": "Cx8sPU5fSmuMfZ4PGis8TQ==", "subType": "04"}}),
            ),
            (
                DocumentNode::Uuid {
                    subtype: 3,
                    value: "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d".into(),
                },
                json!({"$binary": {"base64": "Cx8sPU5fSmuMfZ4PGis8TQ==", "subType": "03"}}),
            ),
            (
                DocumentNode::Timestamp("1767225600:1".into()),
                json!({"$timestamp": {"t": 1_767_225_600u32, "i": 1}}),
            ),
            (DocumentNode::MinKey, json!({"$minKey": 1})),
            (DocumentNode::MaxKey, json!({"$maxKey": 1})),
            (
                DocumentNode::Opaque {
                    type_name: "regex".into(),
                    len: 10,
                },
                json!("<regex len=10>"),
            ),
        ];
        for (node, expected) in cases {
            assert_eq!(render(node.clone()), expected, "{node:?}");
        }
    }

    #[test]
    fn un_document_garde_l_ordre_de_ses_champs() {
        let node = DocumentNode::Object(vec![
            ("z".into(), DocumentNode::Int(1)),
            (
                "a".into(),
                DocumentNode::Array(vec![DocumentNode::Text("x".into())]),
            ),
        ]);
        assert_eq!(
            serde_json::to_string(&render(node)).unwrap(),
            r#"{"z":1,"a":["x"]}"#
        );
    }
}
