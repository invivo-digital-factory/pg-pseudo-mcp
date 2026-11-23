use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::{Cell, QueryOutput};

pub const ENUM_LABELS_SQL: &str = "SELECT DISTINCT enumlabel::text FROM pg_catalog.pg_enum";

pub fn read_enum_labels(output: &QueryOutput) -> Result<Vec<String>, SessionError> {
    output
        .rows
        .iter()
        .map(|row| match row.as_slice() {
            [Cell::Text(label)] => Ok(label.clone()),
            _ => Err(SessionError::Decode(
                "the enum label listing has an unexpected shape".to_string(),
            )),
        })
        .collect()
}

#[cfg(test)]
mod unit {
    use super::*;

    fn output(rows: Vec<Vec<Cell>>) -> QueryOutput {
        QueryOutput {
            columns: Vec::new(),
            rows,
            more_rows: false,
            notices: Vec::new(),
        }
    }

    #[test]
    fn les_libelles_d_enum_sont_rendus_tels_quels() {
        let labels = read_enum_labels(&output(vec![
            vec![Cell::Text("AUTHORIZED".into())],
            vec![Cell::Text("BANK_CARD".into())],
        ]))
        .unwrap();
        assert_eq!(labels, ["AUTHORIZED", "BANK_CARD"]);
    }

    #[test]
    fn une_ligne_de_libelle_de_forme_inattendue_est_refusee() {
        let error = read_enum_labels(&output(vec![vec![Cell::Int(1)]])).unwrap_err();
        assert!(matches!(error, SessionError::Decode(_)));
    }
}
