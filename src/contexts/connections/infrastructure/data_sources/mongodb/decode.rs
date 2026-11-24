use mongodb::bson::spec::BinarySubtype;
use mongodb::bson::{Binary, Bson, Document};

use crate::contexts::connections::domain::types::{DocumentNode, UuidSubtype};

const UUID_LEN: usize = 16;
const OBJECT_ID_LEN: usize = 12;
const MILLIS_PER_DAY: i64 = 86_400_000;
const FIRST_ISO_MILLIS: i64 = -62_167_219_200_000;
const LAST_ISO_MILLIS: i64 = 253_402_300_799_999;

pub fn decode_document(document: Document) -> DocumentNode {
    DocumentNode::Object(
        document
            .into_iter()
            .map(|(key, value)| (key, decode(value)))
            .collect(),
    )
}

pub fn decode(value: Bson) -> DocumentNode {
    match value {
        Bson::Null | Bson::Undefined => DocumentNode::Null,
        Bson::Boolean(flag) => DocumentNode::Bool(flag),
        Bson::Int32(number) => DocumentNode::Int(i64::from(number)),
        Bson::Int64(number) => DocumentNode::Int(number),
        Bson::Double(number) => DocumentNode::Double(number),
        Bson::Decimal128(number) => DocumentNode::Decimal(number.to_string()),
        Bson::String(text) | Bson::Symbol(text) => DocumentNode::Text(text),
        Bson::DateTime(date) => DocumentNode::Date(date_text(date.timestamp_millis())),
        Bson::ObjectId(id) => DocumentNode::ObjectId(id.to_hex()),
        Bson::Timestamp(stamp) => {
            DocumentNode::Timestamp(format!("{}:{}", stamp.time, stamp.increment))
        }
        Bson::MinKey => DocumentNode::MinKey,
        Bson::MaxKey => DocumentNode::MaxKey,
        Bson::Binary(binary) => decode_binary(binary),
        Bson::RegularExpression(regex) => opaque("regex", regex.pattern.chars().count()),
        Bson::JavaScriptCode(code) => opaque("javascript", code.chars().count()),
        Bson::JavaScriptCodeWithScope(code) => opaque("javascript", code.code.chars().count()),
        pointer @ Bson::DbPointer(_) => opaque("dbpointer", pointer_len(pointer)),
        Bson::Array(items) => DocumentNode::Array(items.into_iter().map(decode).collect()),
        Bson::Document(document) => decode_document(document),
    }
}

fn decode_binary(binary: Binary) -> DocumentNode {
    let subtype = match binary.subtype {
        BinarySubtype::Uuid => Some(UuidSubtype::Standard),
        BinarySubtype::UuidOld => Some(UuidSubtype::Legacy),
        _ => None,
    };
    if let Some(subtype) = subtype.filter(|_| binary.bytes.len() == UUID_LEN) {
        return DocumentNode::Uuid {
            subtype,
            value: uuid_text(&binary.bytes),
        };
    }
    DocumentNode::Binary {
        subtype: u8::from(binary.subtype),
        len: binary.bytes.len(),
    }
}

fn uuid_text(bytes: &[u8]) -> String {
    let hex = hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn opaque(type_name: &str, len: usize) -> DocumentNode {
    DocumentNode::Opaque {
        type_name: type_name.to_string(),
        len,
    }
}

fn pointer_len(pointer: Bson) -> usize {
    let namespace_len = pointer
        .into_relaxed_extjson()
        .pointer("/$dbPointer/$ref")
        .and_then(serde_json::Value::as_str)
        .map_or(0, |namespace| namespace.chars().count());
    namespace_len + OBJECT_ID_LEN
}

fn date_text(millis: i64) -> String {
    if !(FIRST_ISO_MILLIS..=LAST_ISO_MILLIS).contains(&millis) {
        return millis.to_string();
    }
    let days = millis.div_euclid(MILLIS_PER_DAY);
    let of_day = millis.rem_euclid(MILLIS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        of_day / 3_600_000,
        of_day / 60_000 % 60,
        of_day / 1_000 % 60,
        of_day % 1_000
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod unit {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use mongodb::bson::spec::BinarySubtype;
    use mongodb::bson::{
        Binary, DateTime, Decimal128, JavaScriptCodeWithScope, Regex, Timestamp, doc, oid::ObjectId,
    };
    use serde_json::{Value, json};

    use super::*;

    const TOKEN: &str = "0b1f2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

    fn uuid_bytes(text: &str) -> Vec<u8> {
        hex::decode(text.replace('-', "")).expect("uuid hex")
    }

    fn binary(subtype: BinarySubtype, bytes: Vec<u8>) -> Bson {
        Bson::Binary(Binary { subtype, bytes })
    }

    fn seeded_event() -> Document {
        doc! {
            "_id": 1,
            "kind": "payment",
            "amount": "12345.6700".parse::<Decimal128>().expect("decimal"),
            "telephone": 33_641_278_395_i64,
            "quantity": 2_i64,
            "payload": binary(BinarySubtype::Generic, vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a]),
            "token": binary(BinarySubtype::Uuid, uuid_bytes(TOKEN)),
            "pattern": Regex { pattern: "^FR76 3000".into(), options: "i".into() },
            "seenAt": Timestamp { time: 1_767_225_600, increment: 1 },
            "at": DateTime::from_millis(1_772_357_400_000),
            "ratio": 0.25,
            "flag": true,
            "missing": Bson::Null,
        }
    }

    fn field<'a>(node: &'a DocumentNode, key: &str) -> &'a DocumentNode {
        let DocumentNode::Object(fields) = node else {
            panic!("not an object: {node:?}");
        };
        &fields
            .iter()
            .find(|(name, _)| name == key)
            .unwrap_or_else(|| panic!("no field {key}"))
            .1
    }

    #[test]
    fn chaque_type_bson_de_la_collection_events_donne_le_noeud_attendu() {
        let node = decode_document(seeded_event());
        let expected = [
            ("_id", DocumentNode::Int(1)),
            ("kind", DocumentNode::Text("payment".into())),
            ("amount", DocumentNode::Decimal("12345.6700".into())),
            ("telephone", DocumentNode::Int(33_641_278_395)),
            ("quantity", DocumentNode::Int(2)),
            ("payload", DocumentNode::Binary { subtype: 0, len: 6 }),
            (
                "token",
                DocumentNode::Uuid {
                    subtype: UuidSubtype::Standard,
                    value: TOKEN.into(),
                },
            ),
            (
                "pattern",
                DocumentNode::Opaque {
                    type_name: "regex".into(),
                    len: 10,
                },
            ),
            ("seenAt", DocumentNode::Timestamp("1767225600:1".into())),
            ("at", DocumentNode::Date("2026-03-01T09:30:00.000Z".into())),
            ("ratio", DocumentNode::Double(0.25)),
            ("flag", DocumentNode::Bool(true)),
            ("missing", DocumentNode::Null),
        ];
        for (key, node_expected) in &expected {
            assert_eq!(field(&node, key), node_expected, "{key}");
        }
        let DocumentNode::Object(fields) = &node else {
            panic!("object");
        };
        let order: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
        let expected_order: Vec<&str> = expected.iter().map(|(name, _)| *name).collect();
        assert_eq!(order, expected_order);
    }

    #[test]
    fn les_types_bson_rares_ont_chacun_leur_noeud() {
        let pointer = Bson::try_from(json!({
            "$dbPointer": {"$ref": "shop.customers", "$id": {"$oid": "65a000000000000000000001"}}
        }))
        .expect("db pointer");
        let cases = [
            (Bson::Int32(-7), DocumentNode::Int(-7)),
            (
                Bson::Symbol("legacy".into()),
                DocumentNode::Text("legacy".into()),
            ),
            (Bson::Undefined, DocumentNode::Null),
            (Bson::MinKey, DocumentNode::MinKey),
            (Bson::MaxKey, DocumentNode::MaxKey),
            (
                Bson::ObjectId(ObjectId::parse_str("65a000000000000000000001").expect("oid")),
                DocumentNode::ObjectId("65a000000000000000000001".into()),
            ),
            (
                Bson::JavaScriptCode("return 1;".into()),
                DocumentNode::Opaque {
                    type_name: "javascript".into(),
                    len: 9,
                },
            ),
            (
                Bson::JavaScriptCodeWithScope(JavaScriptCodeWithScope {
                    code: "return x;".into(),
                    scope: doc! {"x": "secret"},
                }),
                DocumentNode::Opaque {
                    type_name: "javascript".into(),
                    len: 9,
                },
            ),
            (
                pointer,
                DocumentNode::Opaque {
                    type_name: "dbpointer".into(),
                    len: 26,
                },
            ),
            (
                binary(BinarySubtype::UuidOld, uuid_bytes(TOKEN)),
                DocumentNode::Uuid {
                    subtype: UuidSubtype::Legacy,
                    value: TOKEN.into(),
                },
            ),
            (
                binary(BinarySubtype::Uuid, vec![1, 2, 3]),
                DocumentNode::Binary { subtype: 4, len: 3 },
            ),
            (
                binary(BinarySubtype::UserDefined(0x80), vec![9; 40]),
                DocumentNode::Binary {
                    subtype: 0x80,
                    len: 40,
                },
            ),
            (
                Bson::Array(vec![Bson::Int64(1), Bson::Document(doc! {"a": "b"})]),
                DocumentNode::Array(vec![
                    DocumentNode::Int(1),
                    DocumentNode::Object(vec![("a".into(), DocumentNode::Text("b".into()))]),
                ]),
            ),
        ];
        for (value, expected) in cases {
            assert_eq!(decode(value.clone()), expected, "{value:?}");
        }
    }

    #[test]
    fn une_date_est_rendue_en_iso_8601_utc_a_la_milliseconde() {
        let cases = [
            (1_772_357_400_000, "2026-03-01T09:30:00.000Z"),
            (-616_852_799_877, "1950-06-15T12:00:00.123Z"),
            (0, "1970-01-01T00:00:00.000Z"),
            (-1, "1969-12-31T23:59:59.999Z"),
            (-62_167_219_200_000, "0000-01-01T00:00:00.000Z"),
            (253_402_300_799_999, "9999-12-31T23:59:59.999Z"),
        ];
        for (millis, text) in cases {
            assert_eq!(
                decode(Bson::DateTime(DateTime::from_millis(millis))),
                DocumentNode::Date(text.into()),
                "{millis}"
            );
        }
    }

    #[test]
    fn une_date_hors_de_la_plage_iso_est_rendue_en_millisecondes() {
        for millis in [253_402_300_800_000, -62_167_219_200_001, i64::MAX, i64::MIN] {
            assert_eq!(
                decode(Bson::DateTime(DateTime::from_millis(millis))),
                DocumentNode::Date(millis.to_string()),
                "{millis}"
            );
        }
    }

    fn relaxed(node: &DocumentNode) -> Value {
        match node {
            DocumentNode::Null => Value::Null,
            DocumentNode::Bool(value) => json!(value),
            DocumentNode::Int(value) => json!(value),
            DocumentNode::Double(value) if value.is_finite() => json!(value),
            DocumentNode::Double(value) if value.is_nan() => json!({"$numberDouble": "NaN"}),
            DocumentNode::Double(value) if *value > 0.0 => json!({"$numberDouble": "Infinity"}),
            DocumentNode::Double(_) => json!({"$numberDouble": "-Infinity"}),
            DocumentNode::Decimal(text) => json!({"$numberDecimal": text}),
            DocumentNode::Text(text) => json!(text),
            DocumentNode::Date(text) if text.contains('T') => json!({"$date": text}),
            DocumentNode::Date(text) => json!({"$date": {"$numberLong": text}}),
            DocumentNode::ObjectId(text) => json!({"$oid": text}),
            DocumentNode::Uuid {
                subtype: UuidSubtype::Standard,
                value,
            } => json!({"$uuid": value}),
            DocumentNode::Uuid {
                subtype: UuidSubtype::Legacy,
                value,
            } => json!({"$binary": {
                "base64": BASE64_STANDARD.encode(uuid_bytes(value)),
                "subType": "03",
            }}),
            DocumentNode::Timestamp(text) => {
                let (time, increment) = text.split_once(':').expect("t:i");
                json!({"$timestamp": {
                    "t": time.parse::<u32>().expect("t"),
                    "i": increment.parse::<u32>().expect("i"),
                }})
            }
            DocumentNode::MinKey => json!({"$minKey": 1}),
            DocumentNode::MaxKey => json!({"$maxKey": 1}),
            DocumentNode::Array(items) => Value::Array(items.iter().map(relaxed).collect()),
            DocumentNode::Object(fields) => Value::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), relaxed(value)))
                    .collect(),
            ),
            DocumentNode::Binary { .. } | DocumentNode::Opaque { .. } => {
                panic!("lossy node {node:?}")
            }
        }
    }

    #[test]
    fn un_noeud_rendu_en_extended_json_relache_relit_la_meme_valeur_bson() {
        let lossless = doc! {
            "_id": ObjectId::parse_str("65a000000000000000000001").expect("oid"),
            "int": 42,
            "long": 33_641_278_395_i64,
            "double": 0.25,
            "whole": 3.0,
            "infinite": f64::INFINITY,
            "decimal": "-15.0000".parse::<Decimal128>().expect("decimal"),
            "text": "Ylwenna",
            "flag": false,
            "none": Bson::Null,
            "recent": DateTime::from_millis(1_772_357_400_000),
            "whole_second": DateTime::from_millis(1_772_323_200_000),
            "old": DateTime::from_millis(-616_852_799_877),
            "far": DateTime::from_millis(253_402_300_800_000),
            "token": binary(BinarySubtype::Uuid, uuid_bytes(TOKEN)),
            "legacy_token": binary(BinarySubtype::UuidOld, uuid_bytes(TOKEN)),
            "seen": Timestamp { time: 1_767_225_600, increment: 1 },
            "low": Bson::MinKey,
            "high": Bson::MaxKey,
            "nested": {"list": [1, {"a": "b"}, []]},
        };
        let rendered = relaxed(&decode_document(lossless.clone()));
        let reread = Bson::try_from(rendered.clone()).expect("rendered Extended JSON parses");
        let driver = Bson::try_from(Bson::Document(lossless.clone()).into_relaxed_extjson())
            .expect("driver Extended JSON parses");
        assert_eq!(reread, driver, "{rendered}");
        assert_eq!(reread, Bson::Document(lossless));
    }
}
