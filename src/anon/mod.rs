pub mod apply;
mod convert;
mod lookup;

use crate::contexts::anonymization::domain::types::{ColumnClass, Kind};
use crate::contexts::anonymization::use_cases::classify_column::ClassifyColumn;
use crate::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;

pub use crate::contexts::anonymization::domain::types::ColumnInfo;

pub fn classify_column(info: &ColumnInfo) -> ColumnClass {
    ClassifyColumn.execute(info)
}

pub fn dictionary_kind(column_name: &str) -> Option<Kind> {
    DictionaryKind.execute(column_name)
}
