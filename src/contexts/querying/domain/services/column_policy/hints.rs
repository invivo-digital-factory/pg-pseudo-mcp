use crate::contexts::querying::domain::constants::catalog::DETECTED_PER_VALUE;
use crate::contexts::querying::domain::constants::column_policy::POLICY_HINT;
use crate::contexts::querying::domain::types::{
    ColumnClassification, ColumnEntry, ColumnFlag, ColumnMeta, ColumnPolicy, ColumnPolicyHint,
    Reason,
};

pub fn column_hint(column: String, reason: Reason) -> Option<ColumnPolicyHint> {
    matches!(reason, Reason::Dictionary | Reason::Default).then_some(ColumnPolicyHint {
        column,
        reason,
        hint: POLICY_HINT,
    })
}

pub fn describe_hint(
    column: String,
    class: &ColumnClassification,
    entry: &ColumnEntry,
) -> Option<ColumnPolicyHint> {
    if entry.policy.is_some() {
        return None;
    }
    if class.forced_kind.is_some() && entry.pseudonymized.is_some() {
        return column_hint(column, Reason::Dictionary);
    }
    (entry.pseudonymized.as_deref() == Some(DETECTED_PER_VALUE))
        .then(|| column_hint(column, Reason::Default))
        .flatten()
}

pub fn result_hints(
    columns: &[ColumnMeta],
    flags: &[Option<ColumnFlag>],
    policy: &ColumnPolicy,
) -> Vec<ColumnPolicyHint> {
    let mut hints: Vec<ColumnPolicyHint> = Vec::new();
    for (column, flag) in columns.iter().zip(flags) {
        let (Some(source), Some(flag)) = (&column.source, flag) else {
            continue;
        };
        if policy.decide(source).is_some() {
            continue;
        }
        let name = format!("{}.{}.{}", source.schema, source.table, source.column);
        if hints.iter().any(|hint| hint.column == name) {
            continue;
        }
        hints.extend(column_hint(name, flag.why));
    }
    hints
}
