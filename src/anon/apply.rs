use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{
    ColumnClass, ColumnInfo, Reason, TextVerdict, classify_column, detect_spans, dictionary_kind,
    judge_text, numeric_shape_kind,
};
use crate::kind::Kind;
use crate::pg::{Cell, ColumnMeta, DbMessage};
use crate::pseudo::Pseudonymizer;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColumnFlag {
    pub kind: String,
    pub why: Reason,
}

pub struct Anonymizer<'a> {
    pseudonymizer: &'a Pseudonymizer,
    injected: Vec<(String, String)>,
}

pub struct AnonymizedRows {
    pub rows: Vec<Vec<Value>>,
    pub pseudonymized: BTreeMap<String, ColumnFlag>,
}

impl<'a> Anonymizer<'a> {
    pub fn new(pseudonymizer: &'a Pseudonymizer, injected: &[(String, String)]) -> Self {
        let mut injected: Vec<(String, String)> = injected
            .iter()
            .filter(|(real, _)| !real.trim().is_empty())
            .cloned()
            .collect();
        injected.sort_by_key(|(real, _)| std::cmp::Reverse(real.len()));
        Self {
            pseudonymizer,
            injected,
        }
    }

    pub fn rows(&self, columns: &[ColumnMeta], rows: Vec<Vec<Cell>>) -> AnonymizedRows {
        let classes: Vec<ColumnClass> = columns
            .iter()
            .map(|column| {
                classify_column(&ColumnInfo {
                    column_name: Some(
                        column
                            .source
                            .as_ref()
                            .map_or(column.name.as_str(), |source| source.column.as_str()),
                    ),
                    type_name: &column.type_name,
                    derived: column.source.is_none(),
                })
            })
            .collect();
        let mut flags: Vec<Option<ColumnFlag>> = vec![None; columns.len()];
        let rows = rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .enumerate()
                    .map(|(index, cell)| {
                        let mut tracker = Tracker::default();
                        let value = self.cell(cell, &classes[index], &mut tracker);
                        if let Some((kind, reason)) = tracker.first {
                            let why = if classes[index].derived {
                                Reason::Derived
                            } else {
                                reason
                            };
                            flags[index].get_or_insert(ColumnFlag {
                                kind: kind.to_string(),
                                why,
                            });
                        }
                        value
                    })
                    .collect()
            })
            .collect();
        let pseudonymized = columns
            .iter()
            .zip(flags)
            .filter_map(|(column, flag)| flag.map(|flag| (column.name.clone(), flag)))
            .collect();
        AnonymizedRows {
            rows,
            pseudonymized,
        }
    }

    pub fn free_text(&self, text: &str) -> String {
        let mut tracker = Tracker::default();
        let scrubbed = self.scrub_injected(text, &mut tracker);
        self.replace_spans(&scrubbed, &detect_spans(&scrubbed), &mut tracker)
    }

    pub fn message(&self, message: &DbMessage) -> DbMessage {
        let quotes_data = message
            .sqlstate
            .as_deref()
            .is_some_and(|state| state.starts_with("22") || state.starts_with("23"));
        let scrub = |text: &str| {
            let text = self.free_text(text);
            if quotes_data {
                self.mask_quoted(&text)
            } else {
                text
            }
        };
        DbMessage {
            severity: message.severity.clone(),
            sqlstate: message.sqlstate.clone(),
            message: scrub(&message.message),
            detail: message.detail.as_deref().map(scrub),
            hint: message.hint.as_deref().map(scrub),
            where_: None,
        }
    }

    fn mask_quoted(&self, text: &str) -> String {
        QUOTED
            .replace_all(text, |captures: &regex::Captures| {
                let inner = captures
                    .get(2)
                    .or_else(|| captures.get(4))
                    .map_or("", |m| m.as_str());
                if inner.is_empty() || self.is_pseudonym(inner) {
                    captures[0].to_string()
                } else {
                    let token = self.pseudonymizer.opaque_token("value", inner);
                    match captures.get(1) {
                        Some(_) => format!("\"{token}\""),
                        None => format!("=({token})"),
                    }
                }
            })
            .into_owned()
    }

    fn cell(&self, cell: Cell, class: &ColumnClass, tracker: &mut Tracker) -> Value {
        match cell {
            Cell::Null => Value::Null,
            Cell::Bool(value) => Value::Bool(value),
            Cell::Int(value) => self.number(&value.to_string(), json!(value), class, tracker),
            Cell::Float(value) => {
                if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e18 {
                    let digits = format!("{}", value as i64);
                    self.number(&digits, json!(value), class, tracker)
                } else {
                    json!(value)
                }
            }
            Cell::Numeric(text) => {
                let original = Value::String(text.clone());
                self.number(&text, original, class, tracker)
            }
            Cell::Text(text) => Value::String(self.text(&text, class, tracker)),
            Cell::Json(value) => self.json(value, None, tracker),
            Cell::Uuid(text) | Cell::Time(text) | Cell::Interval(text) => Value::String(text),
            Cell::Date(text) if class.derived && class.forced_kind.is_none() => {
                Value::String(self.pseudonymize(Kind::BirthDate, &text, Reason::Derived, tracker))
            }
            Cell::Date(text) | Cell::Timestamp(text) | Cell::TimestampTz(text) => {
                if class.forced_kind == Some(Kind::BirthDate) {
                    Value::String(self.pseudonymize(
                        Kind::BirthDate,
                        &text,
                        Reason::Dictionary,
                        tracker,
                    ))
                } else {
                    Value::String(text)
                }
            }
            Cell::Inet(text) => {
                Value::String(self.pseudonymize(Kind::IpAddress, &text, Reason::Detected, tracker))
            }
            Cell::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(|item| self.cell(item, class, tracker))
                    .collect(),
            ),
            Cell::Composite(fields) => {
                let mut object = Map::new();
                for (name, field) in fields {
                    let field_class = classify_column(&ColumnInfo {
                        column_name: Some(&name),
                        type_name: field_type(&field),
                        derived: class.derived,
                    });
                    object.insert(name, self.cell(field, &field_class, tracker));
                }
                Value::Object(object)
            }
            Cell::Opaque { type_name, len } => {
                tracker.note(Kind::PersonName, Reason::Default, false);
                Value::String(format!("<{type_name} len={len}>"))
            }
        }
    }

    fn number(
        &self,
        digits: &str,
        original: Value,
        class: &ColumnClass,
        tracker: &mut Tracker,
    ) -> Value {
        let hint = match class.forced_kind {
            Some(hint) => hint,
            None if class.derived => {
                let Some(kind) = DERIVED_NUMBER_KINDS
                    .iter()
                    .find_map(|kind| numeric_shape_kind(digits, Some(*kind)))
                else {
                    return original;
                };
                kind
            }
            None => return original,
        };
        let kind = numeric_shape_kind(digits, Some(hint)).unwrap_or(hint);
        let integral = digits.split('.').next().unwrap_or(digits);
        let pseudonym = self.pseudonymize(kind, integral, Reason::Dictionary, tracker);
        if original.is_number()
            && let Ok(number) = pseudonym.parse::<i64>()
        {
            return json!(number);
        }
        Value::String(pseudonym)
    }

    fn text(&self, text: &str, class: &ColumnClass, tracker: &mut Tracker) -> String {
        let scrubbed = self.scrub_injected(text, tracker);
        if self.is_pseudonym(scrubbed.trim()) {
            return scrubbed;
        }
        match judge_text(&scrubbed, class) {
            TextVerdict::Clear => scrubbed,
            TextVerdict::Spans(spans) if class.derived && leaves_words(&scrubbed, &spans) => {
                self.pseudonymize(spans[0].kind, &scrubbed, Reason::Derived, tracker)
            }
            TextVerdict::Spans(spans) => self.replace_spans(&scrubbed, &spans, tracker),
            TextVerdict::WholeValue { kind, reason } => {
                self.pseudonymize(kind, &scrubbed, reason, tracker)
            }
            TextVerdict::Digest => {
                tracker.note(Kind::PersonName, Reason::Derived, false);
                self.pseudonymizer.opaque_token("digest", &scrubbed)
            }
        }
    }

    fn json(&self, value: Value, key_kind: Option<Kind>, tracker: &mut Tracker) -> Value {
        match value {
            Value::String(text) => {
                let class = ColumnClass {
                    forced_kind: key_kind,
                    clear_by_type: false,
                    derived: false,
                };
                Value::String(self.text(&text, &class, tracker))
            }
            Value::Number(number) => match key_kind {
                Some(kind) => {
                    let class = ColumnClass {
                        forced_kind: Some(kind),
                        clear_by_type: false,
                        derived: false,
                    };
                    let text = number.to_string();
                    self.number(&text, Value::Number(number), &class, tracker)
                }
                None => Value::Number(number),
            },
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(|item| self.json(item, key_kind, tracker))
                    .collect(),
            ),
            Value::Object(entries) => Value::Object(
                entries
                    .into_iter()
                    .map(|(key, item)| {
                        let kind = dictionary_kind(&key);
                        let key = self.free_text_tracked(&key, tracker);
                        let item = self.json(item, kind, tracker);
                        (key, item)
                    })
                    .collect(),
            ),
            other => other,
        }
    }

    fn free_text_tracked(&self, text: &str, tracker: &mut Tracker) -> String {
        let scrubbed = self.scrub_injected(text, tracker);
        self.replace_spans(&scrubbed, &detect_spans(&scrubbed), tracker)
    }

    fn replace_spans(&self, text: &str, spans: &[super::Span], tracker: &mut Tracker) -> String {
        let mut output = text.to_string();
        let mut ordered = spans.to_vec();
        ordered.sort_by_key(|span| std::cmp::Reverse(span.start));
        for span in ordered {
            let Some(fragment) = text.get(span.start..span.end) else {
                continue;
            };
            if self.is_pseudonym(fragment) {
                continue;
            }
            let pseudonym = self.pseudonymize(span.kind, fragment, Reason::Detected, tracker);
            output.replace_range(span.start..span.end, &pseudonym);
        }
        output
    }

    fn is_pseudonym(&self, text: &str) -> bool {
        self.injected.iter().any(|(_, pseudonym)| pseudonym == text)
            || self.pseudonymizer.looks_like_pseudonym(text).is_some()
    }

    fn scrub_injected(&self, text: &str, tracker: &mut Tracker) -> String {
        let mut output = text.to_string();
        for (real, pseudonym) in &self.injected {
            if let Some(replaced) = replace_case_insensitive(&output, real, pseudonym) {
                output = replaced;
                tracker.note(Kind::PersonName, Reason::Detected, true);
            }
        }
        output
    }

    fn pseudonymize(
        &self,
        kind: Kind,
        value: &str,
        reason: Reason,
        tracker: &mut Tracker,
    ) -> String {
        tracker.note(kind, reason, false);
        self.pseudonymizer.pseudonymize(kind, value).text
    }
}

const DERIVED_WHOLE_VALUE_WORDS: usize = 6;

const DERIVED_NUMBER_KINDS: [Kind; 5] = [
    Kind::Phone,
    Kind::Siret,
    Kind::Siren,
    Kind::Nir,
    Kind::CardNumber,
];

static QUOTED: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r#"(")([^"]*)(")|=\(([^)]*)\)"#).expect("quoted fragment pattern")
});

fn leaves_words(text: &str, spans: &[super::Span]) -> bool {
    if text.split_whitespace().count() > DERIVED_WHOLE_VALUE_WORDS {
        return false;
    }
    let mut residue = String::with_capacity(text.len());
    let mut cursor = 0;
    for span in spans {
        if span.start >= cursor {
            residue.push_str(text.get(cursor..span.start).unwrap_or(""));
            residue.push(' ');
            cursor = span.end;
        }
    }
    residue.push_str(text.get(cursor..).unwrap_or(""));
    residue
        .split(|c: char| !c.is_alphabetic())
        .any(|word| word.chars().count() >= 3)
}

#[derive(Default)]
struct Tracker {
    first: Option<(&'static str, Reason)>,
}

impl Tracker {
    fn note(&mut self, kind: Kind, reason: Reason, translated: bool) {
        if self.first.is_none() {
            let label = if translated {
                "translated"
            } else {
                kind.as_str()
            };
            self.first = Some((label, reason));
        }
    }
}

fn field_type(cell: &Cell) -> &'static str {
    match cell {
        Cell::Int(_) | Cell::Float(_) | Cell::Numeric(_) => "numeric",
        Cell::Bool(_) => "bool",
        Cell::Uuid(_) => "uuid",
        Cell::Date(_) => "date",
        Cell::Timestamp(_) | Cell::TimestampTz(_) => "timestamp",
        _ => "text",
    }
}

fn replace_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> Option<String> {
    let pattern = regex::RegexBuilder::new(&regex::escape(needle))
        .case_insensitive(true)
        .build()
        .ok()?;
    if !pattern.is_match(haystack) {
        return None;
    }
    Some(
        pattern
            .replace_all(haystack, regex::NoExpand(replacement))
            .into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::replace_case_insensitive;

    #[test]
    fn replaces_every_case_variant_of_the_needle() {
        assert_eq!(
            replace_case_insensitive("A b@C.fr and B@c.FR", "b@c.fr", "x@y.org").as_deref(),
            Some("A x@y.org and x@y.org")
        );
        assert_eq!(
            replace_case_insensitive("İstanbul MAËLYS", "maëlys", "x").as_deref(),
            Some("İstanbul x")
        );
        assert_eq!(replace_case_insensitive("nothing", "b@c.fr", "x"), None);
    }
}
