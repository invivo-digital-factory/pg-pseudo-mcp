use mongodb::bson::{Bson, Document};
use mongodb::results::CollectionType;

use super::sample::bson_type_name;

use crate::contexts::connections::domain::types::{CollectionKind, IndexKeys};

const JSON_SCHEMA: &str = "$jsonSchema";
const PROPERTIES: &str = "properties";
const ITEMS: &str = "items";

pub fn collection_kind(collection_type: &CollectionType) -> CollectionKind {
    match collection_type {
        CollectionType::View => CollectionKind::View,
        CollectionType::Timeseries => CollectionKind::Timeseries,
        _ => CollectionKind::Collection,
    }
}

pub fn index_keys(keys: &Document) -> IndexKeys {
    IndexKeys {
        keys: keys
            .iter()
            .map(|(field, direction)| (field.clone(), direction_text(direction)))
            .collect(),
    }
}

pub fn validator_fields(validator: &Document) -> Vec<String> {
    let mut fields = Vec::new();
    if let Ok(schema) = validator.get_document(JSON_SCHEMA) {
        schema_fields(schema, "", &mut fields);
    }
    fields.sort();
    fields.dedup();
    fields
}

fn schema_fields(schema: &Document, prefix: &str, fields: &mut Vec<String>) {
    if let Ok(properties) = schema.get_document(PROPERTIES) {
        for (name, property) in properties {
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}.{name}")
            };
            fields.push(path.clone());
            if let Bson::Document(property) = property {
                schema_fields(property, &path, fields);
            }
        }
    }
    if let Ok(items) = schema.get_document(ITEMS) {
        schema_fields(items, prefix, fields);
    }
}

fn direction_text(direction: &Bson) -> String {
    match direction {
        Bson::Int32(number) => number.to_string(),
        Bson::Int64(number) => number.to_string(),
        Bson::Double(number) if number.fract() == 0.0 => format!("{number:.0}"),
        Bson::String(kind) => kind.clone(),
        other => bson_type_name(other).to_string(),
    }
}

#[cfg(test)]
mod unit {
    use mongodb::bson::doc;

    use super::*;

    #[test]
    fn les_cles_d_index_rendent_le_champ_et_le_sens_sans_autre_option() {
        let keys = doc! {"contact.email": 1, "placedAt": -1_i64, "loc": "2dsphere", "note": "text", "w": 1.0};
        assert_eq!(
            index_keys(&keys),
            IndexKeys {
                keys: vec![
                    ("contact.email".into(), "1".into()),
                    ("placedAt".into(), "-1".into()),
                    ("loc".into(), "2dsphere".into()),
                    ("note".into(), "text".into()),
                    ("w".into(), "1".into()),
                ]
            }
        );
    }

    #[test]
    fn un_validateur_json_schema_rend_les_chemins_de_ses_proprietes_sans_aucune_valeur() {
        let validator = doc! {
            "$jsonSchema": {
                "bsonType": "object",
                "required": ["email"],
                "properties": {
                    "email": {"bsonType": "string", "pattern": "^jane\\.doe@"},
                    "status": {"enum": ["vip-ylwenna"]},
                    "contact": {
                        "bsonType": "object",
                        "properties": {"phone": {"bsonType": "string"}}
                    },
                    "items": {
                        "bsonType": "array",
                        "items": {"bsonType": "object", "properties": {"label": {}}}
                    }
                }
            }
        };
        let fields = validator_fields(&validator);
        assert_eq!(
            fields,
            vec![
                "contact",
                "contact.phone",
                "email",
                "items",
                "items.label",
                "status"
            ]
        );
        let shown = format!("{fields:?}");
        assert!(
            !shown.contains("jane") && !shown.contains("ylwenna"),
            "{shown}"
        );
    }

    #[test]
    fn un_validateur_sans_json_schema_ne_rend_aucun_champ() {
        assert!(validator_fields(&doc! {"status": {"$in": ["paid", "shipped"]}}).is_empty());
    }

    #[test]
    fn les_types_de_collection_du_pilote_deviennent_des_genres_du_domaine() {
        assert_eq!(
            collection_kind(&CollectionType::Collection),
            CollectionKind::Collection
        );
        assert_eq!(collection_kind(&CollectionType::View), CollectionKind::View);
        assert_eq!(
            collection_kind(&CollectionType::Timeseries),
            CollectionKind::Timeseries
        );
    }
}
