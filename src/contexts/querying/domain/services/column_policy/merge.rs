use std::collections::HashSet;

use super::pattern::parse_pattern;
use crate::contexts::querying::domain::types::{
    ColumnPolicy, ColumnPolicyDocument, ColumnPolicyEntries, ColumnPolicyLoad, PolicyDecision,
};

pub fn merge_policy(
    documents: &[ColumnPolicyDocument],
    is_known_kind: &dyn Fn(&str) -> bool,
) -> ColumnPolicyLoad {
    let mut load = ColumnPolicyLoad::default();
    for document in documents {
        let parsed = document
            .content
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|entries| file_policy(entries, is_known_kind));
        match parsed {
            Ok(policy) => load.policy.extend(policy),
            Err(error) => load.warnings.push(format!(
                "column policy file {} is ignored: {error}",
                document.file
            )),
        }
    }
    load
}

fn file_policy(
    entries: &ColumnPolicyEntries,
    is_known_kind: &dyn Fn(&str) -> bool,
) -> Result<ColumnPolicy, String> {
    let mut policy = ColumnPolicy::default();
    let mut clear = HashSet::new();
    for text in &entries.clear {
        let pattern = parse_pattern(text)?;
        clear.insert(pattern.clone());
        policy.insert(pattern, PolicyDecision::Clear);
    }
    for (text, kind) in &entries.personal {
        let pattern = parse_pattern(text)?;
        if !is_known_kind(kind) {
            return Err(format!("unknown kind `{kind}` for `{text}`"));
        }
        if clear.contains(&pattern) {
            return Err(format!("`{text}` is both clear and personal"));
        }
        policy.insert(pattern, PolicyDecision::Personal(kind.clone()));
    }
    Ok(policy)
}
