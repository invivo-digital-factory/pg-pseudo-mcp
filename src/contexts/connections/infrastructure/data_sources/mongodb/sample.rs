use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use mongodb::bson::{Bson, Document, doc};

use crate::contexts::connections::domain::types::SampledField;

pub const SAMPLE_SIZE: i32 = 100;
pub const SAMPLE_MAX_TIME: Duration = Duration::from_secs(5);

pub fn sample_pipeline() -> Vec<Document> {
    vec![doc! {"$sample": {"size": SAMPLE_SIZE}}]
}

#[derive(Default)]
struct Seen {
    types: BTreeSet<&'static str>,
    documents: usize,
    last_document: Option<usize>,
}

pub fn sampled_fields(documents: &[Document]) -> Vec<SampledField> {
    let mut seen: BTreeMap<Vec<String>, Seen> = BTreeMap::new();
    for (index, document) in documents.iter().enumerate() {
        visit_document(document, &[], index, &mut seen);
    }
    let total = documents.len() as f64;
    seen.into_iter()
        .map(|(path, seen)| SampledField {
            path,
            types: seen.types.into_iter().map(str::to_string).collect(),
            presence: seen.documents as f64 / total,
        })
        .collect()
}

pub(super) fn bson_type_name(value: &Bson) -> &'static str {
    match value {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "object",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        Bson::Int32(_) => "int",
        Bson::Int64(_) => "long",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binData",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Symbol(_) => "symbol",
        Bson::Decimal128(_) => "decimal",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "maxKey",
        Bson::MinKey => "minKey",
        Bson::DbPointer(_) => "dbPointer",
    }
}

fn visit_document(
    document: &Document,
    prefix: &[String],
    index: usize,
    seen: &mut BTreeMap<Vec<String>, Seen>,
) {
    for (key, value) in document {
        let mut path = prefix.to_vec();
        path.push(key.clone());
        visit_value(value, &path, index, seen);
    }
}

fn visit_value(
    value: &Bson,
    path: &[String],
    index: usize,
    seen: &mut BTreeMap<Vec<String>, Seen>,
) {
    let entry = seen.entry(path.to_vec()).or_default();
    entry.types.insert(bson_type_name(value));
    if entry.last_document != Some(index) {
        entry.last_document = Some(index);
        entry.documents += 1;
    }
    match value {
        Bson::Document(document) => visit_document(document, path, index, seen),
        Bson::Array(items) => {
            for item in items {
                seen.entry(path.to_vec())
                    .or_default()
                    .types
                    .insert(bson_type_name(item));
                if let Bson::Document(document) = item {
                    visit_document(document, path, index, seen);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod unit {
    use mongodb::bson::{Bson, DateTime};

    use super::*;

    fn field<'a>(fields: &'a [SampledField], path: &str) -> &'a SampledField {
        fields
            .iter()
            .find(|field| field.path.join(".") == path)
            .unwrap_or_else(|| panic!("no path {path} in {fields:?}"))
    }

    #[test]
    fn le_pipeline_d_echantillon_est_fixe_par_le_serveur() {
        assert_eq!(sample_pipeline(), vec![doc! {"$sample": {"size": 100}}]);
        assert_eq!(SAMPLE_MAX_TIME, Duration::from_secs(5));
    }

    #[test]
    fn les_chemins_imbriques_et_les_tableaux_sont_vus_sans_indice_avec_leur_presence() {
        let documents = vec![
            doc! {
                "firstName": "Ylwenna",
                "contact": {"email": "ylwenna.kervadec@canary-mail.fr", "phones": ["06 41 27 83 95"]},
                "items": [{"product": "Garance Bleue", "quantity": 2_i64}, {"product": "x"}],
                "birthDate": DateTime::from_millis(0),
            },
            doc! {
                "firstName": "Odrigan",
                "contact": {"email": Bson::Null},
                "telephone": 33_758_149_236_i64,
            },
        ];
        let fields = sampled_fields(&documents);
        let paths: Vec<String> = fields.iter().map(|field| field.path.join(".")).collect();
        assert_eq!(
            paths,
            vec![
                "birthDate",
                "contact",
                "contact.email",
                "contact.phones",
                "firstName",
                "items",
                "items.product",
                "items.quantity",
                "telephone",
            ]
        );
        assert_eq!(field(&fields, "firstName").presence, 1.0);
        assert_eq!(field(&fields, "telephone").presence, 0.5);
        assert_eq!(field(&fields, "items.product").presence, 0.5);
        assert_eq!(
            field(&fields, "contact.email").types,
            vec!["null", "string"]
        );
        assert_eq!(
            field(&fields, "contact.phones").types,
            vec!["array", "string"]
        );
        assert_eq!(field(&fields, "items").types, vec!["array", "object"]);
        assert_eq!(field(&fields, "items.quantity").types, vec!["long"]);
        assert_eq!(field(&fields, "birthDate").types, vec!["date"]);
        let shown = format!("{fields:?}");
        for value in [
            "Ylwenna",
            "Odrigan",
            "canary-mail",
            "41 27",
            "Garance",
            "33758149236",
        ] {
            assert!(!shown.contains(value), "{value} leaked in {shown}");
        }
    }

    #[test]
    fn une_cle_dynamique_pointee_reste_un_seul_segment_du_chemin() {
        let documents = vec![doc! {
            "preferences": {"kervadec.ylwenna@canary-mail.fr": {"newsletter": true}},
        }];
        let fields = sampled_fields(&documents);
        let paths: Vec<Vec<String>> = fields.into_iter().map(|field| field.path).collect();
        assert_eq!(
            paths,
            vec![
                vec!["preferences".to_string()],
                vec![
                    "preferences".to_string(),
                    "kervadec.ylwenna@canary-mail.fr".to_string()
                ],
                vec![
                    "preferences".to_string(),
                    "kervadec.ylwenna@canary-mail.fr".to_string(),
                    "newsletter".to_string()
                ],
            ]
        );
    }

    #[test]
    fn un_echantillon_vide_ne_rend_aucun_champ() {
        assert!(sampled_fields(&[]).is_empty());
    }
}
