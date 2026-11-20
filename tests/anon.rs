use pg_pseudo_mcp::contexts::anonymization::domain::services::classify::{
    classify_column, judge_text,
};
use pg_pseudo_mcp::contexts::anonymization::domain::types::{ColumnInfo, TextVerdict};

mod common;

mod unit {
    use super::*;

    #[test]
    fn toute_valeur_canari_est_jugee_personnelle_dans_une_colonne_texte_inconnue() {
        let class = classify_column(&ColumnInfo {
            column_name: Some("body"),
            type_name: "text",
            derived: false,
        });
        for value in common::CANARY_VALUES {
            assert!(
                !matches!(judge_text(value, &class), TextVerdict::Clear),
                "{value}"
            );
        }
    }
}
