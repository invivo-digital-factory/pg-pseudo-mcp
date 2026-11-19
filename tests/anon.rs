use pg_pseudo_mcp::contexts::anonymization::domain::services::classify::{
    classify_column, judge_text,
};
use pg_pseudo_mcp::contexts::anonymization::domain::types::{ColumnInfo, TextVerdict};

mod common;

#[test]
fn every_canary_value_is_judged_personal_in_an_unknown_text_column() {
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
