use std::collections::BTreeSet;

use serde_json::{Map, Number, Value, json};

use crate::contexts::querying::domain::constants::mongo_policy::NOT_TRANSLATED_POSITIONS;
use crate::contexts::querying::domain::types::{Translations, Warning};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralRole {
    Compared,
    ComparedInExpression,
    Other,
    Pattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralSite {
    pub pointer: Vec<Segment>,
    pub role: LiteralRole,
    pub text: String,
    pub numeric: bool,
}

impl LiteralSite {
    fn candidate(&self) -> String {
        match self.role {
            LiteralRole::Pattern => pattern_candidate(&self.text),
            _ => self.text.clone(),
        }
    }
}

pub struct Outcome {
    pub injected: Vec<(String, String)>,
    pub warnings: Vec<Warning>,
}

pub fn lookup_texts(sites: &[LiteralSite]) -> Vec<String> {
    sites
        .iter()
        .map(LiteralSite::candidate)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn translate(
    root: &mut Map<String, Value>,
    sites: &[LiteralSite],
    translations: &Translations,
) -> Outcome {
    let mut injected = Vec::new();
    let mut warned = BTreeSet::new();
    let mut warnings = Vec::new();
    for site in sites {
        let candidate = site.candidate();
        let compared = matches!(
            site.role,
            LiteralRole::Compared | LiteralRole::ComparedInExpression
        );
        match translations.real(&candidate) {
            Some(real) if compared => {
                if let Some(slot) = locate(root, &site.pointer) {
                    *slot = replacement(site, real);
                    injected.push((real.to_string(), candidate));
                }
            }
            Some(_) => {
                if warned.insert(("pseudonym_not_translated", candidate.clone())) {
                    warnings.push(Warning::new(
                        "pseudonym_not_translated",
                        format!(
                            "'{candidate}' is a pseudonym used outside a comparison with a field; {NOT_TRANSLATED_POSITIONS}"
                        ),
                    ));
                }
            }
            None if translations.looks_like_pseudonym(&candidate) => {
                if warned.insert(("unresolved_pseudonym", candidate.clone())) {
                    warnings.push(Warning::new(
                        "unresolved_pseudonym",
                        format!(
                            "'{candidate}' looks like a pseudonym but is not known on this machine; it was compared as written"
                        ),
                    ));
                }
            }
            None => {}
        }
    }
    Outcome { injected, warnings }
}

fn replacement(site: &LiteralSite, real: &str) -> Value {
    if site.numeric
        && real.chars().all(|character| character.is_ascii_digit())
        && let Ok(number) = real.parse::<u64>()
    {
        return Value::Number(Number::from(number));
    }
    if site.role == LiteralRole::ComparedInExpression && real.starts_with('$') {
        return json!({ "$literal": real });
    }
    Value::String(real.to_string())
}

fn locate<'a>(root: &'a mut Map<String, Value>, pointer: &[Segment]) -> Option<&'a mut Value> {
    let (Segment::Key(first), rest) = pointer.split_first()? else {
        return None;
    };
    rest.iter()
        .try_fold(root.get_mut(first)?, |current, segment| match segment {
            Segment::Key(key) => current.as_object_mut()?.get_mut(key),
            Segment::Index(index) => current.as_array_mut()?.get_mut(*index),
        })
}

fn pattern_candidate(pattern: &str) -> String {
    let mut core = pattern.strip_prefix('^').unwrap_or(pattern);
    core = core.strip_prefix(".*").unwrap_or(core);
    if core.ends_with('$') && !core.ends_with("\\$") {
        core = &core[..core.len() - 1];
    }
    core = core.strip_suffix(".*").unwrap_or(core);
    let mut unescaped = String::with_capacity(core.len());
    let mut characters = core.chars().peekable();
    while let Some(character) = characters.next() {
        match characters.peek() {
            Some(next) if character == '\\' && !next.is_alphanumeric() => {}
            _ => unescaped.push(character),
        }
    }
    unescaped
}
