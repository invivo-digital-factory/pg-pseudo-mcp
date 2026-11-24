use crate::contexts::querying::domain::constants::catalog::DETECTED_PER_VALUE;
use crate::contexts::querying::domain::constants::column_policy::{
    HINTED_DICTIONARY_KINDS, POLICY_HINT,
};
use crate::contexts::querying::domain::types::{
    ColumnClassification, ColumnEntry, ColumnFlag, ColumnMeta, ColumnPolicy, ColumnPolicyHint,
    FieldPolicyHint, FieldSource, Reason,
};

fn is_hinted(reason: Reason, kind: &str) -> bool {
    match reason {
        Reason::Default => true,
        Reason::Dictionary => HINTED_DICTIONARY_KINDS.contains(&kind),
        _ => false,
    }
}

pub fn field_hint(
    policy: &ColumnPolicy,
    source: &FieldSource,
    reason: Reason,
    kind: &str,
) -> Option<FieldPolicyHint> {
    if policy.decide_field(source).is_some() {
        return None;
    }
    is_hinted(reason, kind).then(|| FieldPolicyHint {
        collection: format!("{}.{}", source.database, source.collection),
        field: source.path.clone(),
        reason,
        hint: POLICY_HINT,
    })
}

pub fn column_hint(column: String, reason: Reason, kind: &str) -> Option<ColumnPolicyHint> {
    is_hinted(reason, kind).then_some(ColumnPolicyHint {
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
    if let (Some(kind), Some(_)) = (&class.forced_kind, &entry.pseudonymized) {
        return column_hint(column, Reason::Dictionary, kind);
    }
    (entry.pseudonymized.as_deref() == Some(DETECTED_PER_VALUE))
        .then(|| column_hint(column, Reason::Default, DETECTED_PER_VALUE))
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
        hints.extend(column_hint(name, flag.why, &flag.kind));
    }
    hints
}
