use crate::contexts::anonymization::domain::types::ColumnInfo;
use crate::contexts::anonymization::use_cases::classify_column::ClassifyColumn;
use crate::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;
use crate::contexts::querying::domain::repositories::PersonalColumnDictionary;
use crate::contexts::querying::domain::types::ColumnClassification;

pub struct PersonalColumnDictionaryAdapter {
    dictionary_kind: DictionaryKind,
    classify_column: ClassifyColumn,
}

impl PersonalColumnDictionaryAdapter {
    pub fn new(dictionary_kind: DictionaryKind, classify_column: ClassifyColumn) -> Self {
        Self {
            dictionary_kind,
            classify_column,
        }
    }
}

impl PersonalColumnDictionary for PersonalColumnDictionaryAdapter {
    fn dictionary_kind(&self, column_name: &str) -> Option<String> {
        self.dictionary_kind
            .execute(column_name)
            .map(|kind| kind.as_str().to_string())
    }

    fn classify(&self, column_name: &str, type_name: &str) -> ColumnClassification {
        let class = self.classify_column.execute(&ColumnInfo {
            column_name: Some(column_name),
            type_name,
            derived: false,
        });
        ColumnClassification {
            forced_kind: class.forced_kind.map(|kind| kind.as_str().to_string()),
            clear_by_type: class.clear_by_type,
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::querying::domain::services::column_pseudonymization::pseudonymization;

    fn label(name: &str, type_name: &str) -> Option<String> {
        let adapter = PersonalColumnDictionaryAdapter::new(DictionaryKind, ClassifyColumn);
        pseudonymization(&adapter.classify(name, type_name), type_name)
    }

    #[test]
    fn la_pseudonymisation_suit_le_classifieur_de_colonnes_de_requete() {
        assert_eq!(label("email", "text").as_deref(), Some("email"));
        assert_eq!(label("id", "integer"), None);
        assert_eq!(label("created_at", "timestamptz"), None);
        assert_eq!(
            label("label", "varchar(40)").as_deref(),
            Some("detected_per_value")
        );
        assert_eq!(label("origin", "inet").as_deref(), Some("ip_address"));
        assert_eq!(label("flags", "integer[]"), None);
    }
}
