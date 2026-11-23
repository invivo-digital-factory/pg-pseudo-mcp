use crate::contexts::querying::domain::types::{
    Clearance, ColumnMeta, ColumnPolicy, PolicyDecision,
};

pub fn combine_lineage(proven: Vec<Clearance>, with_policy: Vec<Clearance>) -> Vec<Clearance> {
    proven
        .into_iter()
        .zip(with_policy)
        .map(|(proven, with_policy)| match (proven, with_policy) {
            (Clearance::ProvenClear, _) => Clearance::ProvenClear,
            (_, Clearance::ProvenClear) => Clearance::ClearByPolicy,
            (proven, _) => proven,
        })
        .collect()
}

pub fn policy_clearances(
    columns: &[ColumnMeta],
    policy: &ColumnPolicy,
    lineage: Vec<Clearance>,
) -> Vec<Clearance> {
    columns
        .iter()
        .zip(lineage)
        .map(|(column, lineage)| {
            match column
                .source
                .as_ref()
                .and_then(|source| policy.decide(source))
            {
                Some(PolicyDecision::Clear) => Clearance::ClearByPolicy,
                Some(PolicyDecision::Personal(kind)) => Clearance::PersonalByPolicy(kind.clone()),
                None => lineage,
            }
        })
        .collect()
}
