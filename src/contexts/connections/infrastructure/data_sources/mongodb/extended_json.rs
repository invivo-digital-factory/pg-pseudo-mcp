use mongodb::bson::{Bson, Document};
use serde_json::Value;

use super::failure::MongoFailure;

pub fn bson_from_extended_json(value: &Value, part: &str) -> Result<Bson, MongoFailure> {
    Bson::try_from(value.clone())
        .map_err(|_| MongoFailure::InvalidRequest(format!("the {part} is not valid Extended JSON")))
}

pub fn document_from_extended_json(value: &Value, part: &str) -> Result<Document, MongoFailure> {
    let not_a_document = || MongoFailure::InvalidRequest(format!("the {part} must be a document"));
    if !value.is_object() {
        return Err(not_a_document());
    }
    match bson_from_extended_json(value, part)? {
        Bson::Document(document) => Ok(document),
        _ => Err(not_a_document()),
    }
}

#[cfg(test)]
mod unit {
    use mongodb::bson::{DateTime, doc, oid::ObjectId};
    use serde_json::json;

    use super::*;

    fn reason(failure: MongoFailure) -> String {
        match failure {
            MongoFailure::InvalidRequest(reason) => reason,
            MongoFailure::Driver(error) => panic!("unexpected driver error {error}"),
        }
    }

    #[test]
    fn un_filtre_en_extended_json_devient_un_document_bson_type() {
        let filter = json!({
            "_id": {"$oid": "65a000000000000000000001"},
            "at": {"$gte": {"$date": "2026-03-01T00:00:00Z"}},
            "amount": {"$numberDecimal": "12.50"},
            "count": 3,
        });
        let document = document_from_extended_json(&filter, "filter").expect("valid filter");
        assert_eq!(
            document.get("_id"),
            Some(&Bson::ObjectId(
                ObjectId::parse_str("65a000000000000000000001").expect("oid")
            ))
        );
        assert_eq!(
            document.get_document("at").expect("range").get("$gte"),
            Some(&Bson::DateTime(DateTime::from_millis(1_772_323_200_000)))
        );
        assert!(matches!(document.get("amount"), Some(Bson::Decimal128(_))));
        assert_eq!(document.get("count"), Some(&Bson::Int32(3)));
    }

    #[test]
    fn un_extended_json_invalide_est_refuse_sans_recopier_sa_valeur() {
        let filter =
            json!({"email": "jane.doe@canary-mail.fr", "_id": {"$oid": "not-an-oid-jane"}});
        let reason =
            reason(document_from_extended_json(&filter, "filter").expect_err("invalid oid"));
        assert_eq!(reason, "the filter is not valid Extended JSON");
        assert!(!reason.contains("jane"), "{reason}");
    }

    #[test]
    fn une_valeur_qui_n_est_pas_un_document_est_refusee_en_nommant_la_partie() {
        let reason = reason(
            document_from_extended_json(&json!(["a"]), "pipeline stage 2").expect_err("array"),
        );
        assert_eq!(reason, "the pipeline stage 2 must be a document");
    }

    #[test]
    fn une_valeur_extended_json_scalaire_est_convertie() {
        assert_eq!(
            bson_from_extended_json(&json!("idx_email"), "hint").expect("text"),
            Bson::String("idx_email".into())
        );
        assert_eq!(
            bson_from_extended_json(&json!({"email": 1}), "hint").expect("keys"),
            Bson::Document(doc! {"email": 1})
        );
    }
}
