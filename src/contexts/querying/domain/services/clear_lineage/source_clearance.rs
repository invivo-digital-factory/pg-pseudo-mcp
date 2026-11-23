use crate::contexts::querying::domain::repositories::PersonalColumnDictionary;
use crate::contexts::querying::domain::types::CatalogColumn;

pub fn is_clear_source(dictionary: &dyn PersonalColumnDictionary, column: &CatalogColumn) -> bool {
    dictionary.dictionary_kind(&column.name).is_none()
        && dictionary
            .classify(&column.name, &column.type_name)
            .clear_by_type
}
