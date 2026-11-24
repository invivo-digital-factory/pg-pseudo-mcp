use std::collections::HashSet;

use super::pattern::{parse_field_pattern, parse_pattern};
use crate::contexts::querying::domain::types::{
    ColumnPolicy, ColumnPolicyDocument, ColumnPolicyEntries, ColumnPolicyLoad, MongoPolicyEntries,
    PolicyDecision,
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
    mongodb_policy(&entries.mongodb, is_known_kind, &mut policy)?;
    Ok(policy)
}

fn mongodb_policy(
    entries: &MongoPolicyEntries,
    is_known_kind: &dyn Fn(&str) -> bool,
    policy: &mut ColumnPolicy,
) -> Result<(), String> {
    let mut clear = HashSet::new();
    for (key, path) in &entries.clear {
        let pattern = parse_field_pattern(key, path)?;
        clear.insert(pattern.clone());
        policy.insert_field(pattern, PolicyDecision::Clear);
    }
    for (key, path, kind) in &entries.personal {
        let pattern = parse_field_pattern(key, path)?;
        if !is_known_kind(kind) {
            return Err(format!("unknown kind `{kind}` for `{key}` `{path}`"));
        }
        if clear.contains(&pattern) {
            return Err(format!("`{key}` `{path}` is both clear and personal"));
        }
        policy.insert_field(pattern, PolicyDecision::Personal(kind.clone()));
    }
    Ok(())
}
