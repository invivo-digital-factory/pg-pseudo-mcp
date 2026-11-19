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
    pub per_column: Vec<Option<ColumnFlag>>,
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
            .zip(&flags)
            .filter_map(|(column, flag)| flag.clone().map(|flag| (column.name.clone(), flag)))
            .collect();
        let per_column = flags;
        AnonymizedRows {
            rows,
            pseudonymized,
            per_column,
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
    let mut output = String::with_capacity(haystack.len());
    let mut cursor = 0;
    let mut search = 0;
    let mut replaced = false;
    while let Some(found) = pattern.find_at(haystack, search) {
        let before = haystack[..found.start()].chars().next_back();
        let after = haystack[found.end()..].chars().next();
        if before.is_some_and(is_word_char) || after.is_some_and(is_word_char) {
            search = found.start()
                + haystack[found.start()..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
            continue;
        }
        output.push_str(&haystack[cursor..found.start()]);
        output.push_str(replacement);
        cursor = found.end();
        search = cursor;
        replaced = true;
    }
    if !replaced {
        return None;
    }
    output.push_str(&haystack[cursor..]);
    Some(output)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '\u{0300}'..='\u{036F}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pg::ColumnSource;

    fn pseudonymizer() -> (tempfile::TempDir, Pseudonymizer) {
        let dir = tempfile::tempdir().expect("data dir");
        let pseudonymizer = Pseudonymizer::open(dir.path()).expect("pseudonymizer");
        (dir, pseudonymizer)
    }

    fn text_column(name: &str, source: Option<&str>) -> ColumnMeta {
        ColumnMeta {
            name: name.to_string(),
            type_name: "text".to_string(),
            type_oid: 25,
            source: source.map(|column| ColumnSource {
                schema: "public".to_string(),
                table: "customers".to_string(),
                column: column.to_string(),
            }),
        }
    }

    fn message(sqlstate: &str, text: &str, detail: Option<&str>) -> DbMessage {
        DbMessage {
            severity: "ERROR".to_string(),
            sqlstate: Some(sqlstate.to_string()),
            message: text.to_string(),
            detail: detail.map(str::to_string),
            hint: None,
            where_: Some("SQL function \"f\" statement 1".to_string()),
        }
    }

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

    #[test]
    fn an_injected_value_is_replaced_only_where_it_stands_as_a_whole_word() {
        assert_eq!(replace_case_insensitive("Troyes", "Roy", "Zed"), None);
        assert_eq!(
            replace_case_insensitive("leroy.marc@gmail.com", "Roy", "Zed"),
            None
        );
        assert_eq!(replace_case_insensitive("Roy2", "Roy", "Zed"), None);
        assert_eq!(replace_case_insensitive("Royé", "Roy", "Zed"), None);
        assert_eq!(
            replace_case_insensitive("xa-a-a", "a-a", "Z").as_deref(),
            Some("xa-Z")
        );
        assert_eq!(
            replace_case_insensitive("Roy, ROY et roy-Dupont", "Roy", "Zed").as_deref(),
            Some("Zed, Zed et Zed-Dupont")
        );
    }

    #[test]
    fn a_short_injected_name_leaves_longer_words_containing_it_untouched() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let pseudonym = "Zabulon".to_string();
        let injected = [("Roy".to_string(), pseudonym.clone())];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("ville", Some("ville")),
            text_column("commentaire", Some("commentaire")),
            text_column("email", Some("email")),
        ];
        let first = || {
            vec![
                Cell::Text("Troyes".to_string()),
                Cell::Text("Client Roy, appel".to_string()),
                Cell::Text("leroy.marc@gmail.com".to_string()),
            ]
        };
        let rows = vec![
            first(),
            vec![
                Cell::Text("Roy".to_string()),
                Cell::Text("ROY".to_string()),
                Cell::Text(" roy ".to_string()),
            ],
        ];
        let anonymized = anonymizer.rows(&columns, rows);
        let expected = baseline.rows(&columns, vec![first()]);
        assert_eq!(anonymized.rows[0][0], expected.rows[0][0]);
        assert_eq!(anonymized.rows[0][2], expected.rows[0][2]);
        assert_eq!(
            anonymized.rows[0][1],
            json!(format!("Client {pseudonym}, appel"))
        );
        assert_eq!(anonymized.rows[1][0], json!(pseudonym));
        assert_eq!(anonymized.rows[1][1], json!(pseudonym));
        assert_eq!(anonymized.rows[1][2], json!(format!(" {pseudonym} ")));

        let email = anonymizer.free_text("écrire à leroy.marc@gmail.com");
        assert!(!email.contains(&pseudonym), "{email}");
        assert!(!email.contains("leroy.marc@gmail.com"), "{email}");
        assert_eq!(
            anonymizer.free_text("vu Roy à Troyes"),
            format!("vu {pseudonym} à Troyes")
        );
    }

    #[test]
    fn same_named_columns_each_get_their_own_flag() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("email", Some("email")),
            text_column("status", Some("status")),
            text_column("email", Some("email")),
        ];
        let rows = vec![vec![
            Cell::Text("ilona.kervadec@canary-mail.fr".to_string()),
            Cell::Text("PAID".to_string()),
            Cell::Text("odilon.thaumassin@canary-mail.fr".to_string()),
        ]];
        let anonymized = anonymizer.rows(&columns, rows);
        assert_eq!(anonymized.per_column.len(), 3);
        assert!(anonymized.per_column[0].is_some());
        assert!(anonymized.per_column[1].is_none());
        assert!(anonymized.per_column[2].is_some());
        assert_eq!(anonymized.pseudonymized.len(), 1);
    }

    #[test]
    fn invalid_input_messages_quote_opaque_tokens_instead_of_values() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let masked = anonymizer.message(&message(
            "22P02",
            "invalid input syntax for type integer: \"Brizolier\"",
            None,
        ));
        assert!(!masked.message.contains("Brizolier"), "{}", masked.message);
        assert!(
            masked
                .message
                .starts_with("invalid input syntax for type integer: \"<value id="),
            "{}",
            masked.message
        );
        assert_eq!(masked.where_, None);
        assert_eq!(masked.sqlstate.as_deref(), Some("22P02"));

        let pseudonym = pseudonymizer
            .pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr")
            .text;
        let kept = anonymizer.message(&message(
            "22P02",
            &format!("invalid input syntax for type integer: \"{pseudonym}\""),
            None,
        ));
        assert_eq!(
            kept.message,
            format!("invalid input syntax for type integer: \"{pseudonym}\"")
        );
    }

    #[test]
    fn unique_violation_details_mask_the_key_value_unless_it_is_a_pseudonym() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let injected_pseudonym = pseudonymizer.pseudonymize(Kind::LastName, "Kervadec").text;
        let injected = [("Kervadec".to_string(), injected_pseudonym.clone())];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);

        let masked = anonymizer.message(&message(
            "23505",
            "duplicate key value violates unique constraint \"customers_nom_key\"",
            Some("Key (nom)=(Brizolier) already exists."),
        ));
        let detail = masked.detail.expect("detail");
        assert!(!detail.contains("Brizolier"), "{detail}");
        assert!(detail.starts_with("Key (nom)=(<value id="), "{detail}");
        assert!(detail.ends_with(") already exists."), "{detail}");
        assert_eq!(masked.where_, None);

        let translated = anonymizer.message(&message(
            "23505",
            "duplicate key value violates unique constraint \"customers_nom_key\"",
            Some("Key (nom)=(Kervadec) already exists."),
        ));
        assert_eq!(
            translated.detail.as_deref(),
            Some(format!("Key (nom)=({injected_pseudonym}) already exists.").as_str())
        );

        let email = pseudonymizer
            .pseudonymize(Kind::Email, "odilon.thaumassin@canary-mail.fr")
            .text;
        let kept = anonymizer.message(&message(
            "23505",
            "duplicate key value violates unique constraint \"customers_email_key\"",
            Some(&format!("Key (email)=({email}) already exists.")),
        ));
        assert_eq!(
            kept.detail.as_deref(),
            Some(format!("Key (email)=({email}) already exists.").as_str())
        );
    }
}
