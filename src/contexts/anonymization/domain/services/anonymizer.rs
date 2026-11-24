use std::collections::HashSet;

use serde_json::{Map, Value, json};

use super::classify::{classify_column, forced_reason, judge_text, numeric_shape_kind};
use super::detect::{detect_spans, structural_spans};
use super::dictionary::{dictionary_kind, fold};
use super::document_paths::{FieldClearances, PathFlags, Place};
use super::injected_text::replace_injected;
use super::pseudonymizer::Pseudonymizer;
use super::tracker::Tracker;
use crate::contexts::anonymization::domain::types::{
    AnonymizedDocuments, AnonymizedRows, Cell, Clearance, ColumnClass, ColumnFlag, ColumnInfo,
    ColumnMeta, DatabaseMessage, FieldClearance, FieldPath, Kind, Lineage, Node, Reason, Span,
    TextVerdict,
};

pub struct Anonymizer<'a> {
    pseudonymizer: &'a Pseudonymizer,
    injected: Vec<(String, String)>,
    enum_labels: HashSet<String>,
    clearances: Vec<Clearance>,
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
            enum_labels: HashSet::new(),
            clearances: Vec::new(),
        }
    }

    pub fn with_enum_labels(mut self, labels: &[String]) -> Self {
        self.enum_labels = labels
            .iter()
            .map(|label| fold(label.trim()))
            .filter(|label| !label.is_empty())
            .collect();
        self
    }

    pub fn with_clearances(mut self, clearances: &[Clearance]) -> Self {
        self.clearances = clearances.to_vec();
        self
    }

    pub fn rows(
        &self,
        columns: &[ColumnMeta],
        rows: Vec<Vec<Cell>>,
        lineage: Option<&[Lineage]>,
    ) -> AnonymizedRows {
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
        let tainted = tainted_positions(lineage, columns.len());
        let tainted: Vec<bool> = tainted
            .into_iter()
            .zip(&classes)
            .map(|(flag, class)| flag && class.derived)
            .collect();
        let classes: Vec<ColumnClass> = classes
            .into_iter()
            .enumerate()
            .map(|(index, class)| self.cleared(index, &columns[index], class, tainted[index]))
            .collect();
        let tainted: Vec<bool> = tainted
            .into_iter()
            .zip(&classes)
            .map(|(flag, class)| flag && !class.structural_only)
            .collect();
        let mut flags: Vec<Option<ColumnFlag>> = vec![None; columns.len()];
        let rows = rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .enumerate()
                    .map(|(index, cell)| {
                        let mut tracker = Tracker::default();
                        let value = self.cell(cell, &classes[index], tainted[index], &mut tracker);
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

    pub fn documents(
        &self,
        documents: Vec<Node>,
        clearances: &[(FieldPath, FieldClearance)],
    ) -> AnonymizedDocuments {
        let clearances = FieldClearances::new(clearances);
        let root = Place::default();
        let mut flags = PathFlags::default();
        let documents = documents
            .into_iter()
            .map(|document| self.node(document, &root, &clearances, &mut flags))
            .collect();
        let (pseudonymized, omitted_paths) = flags.into_parts();
        AnonymizedDocuments {
            documents,
            pseudonymized,
            omitted_paths,
        }
    }

    fn node(
        &self,
        node: Node,
        place: &Place,
        clearances: &FieldClearances,
        flags: &mut PathFlags,
    ) -> Node {
        match node {
            Node::Array(items) => Node::Array(
                items
                    .into_iter()
                    .map(|item| self.node(item, place, clearances, flags))
                    .collect(),
            ),
            Node::Object(entries) => {
                let (class, tainted) = place.class(DOCUMENT_TEXT);
                Node::Object(
                    entries
                        .into_iter()
                        .map(|(key, value)| {
                            let mut tracker = Tracker::default();
                            let shown =
                                self.json_key(&key, class.structural_only, tainted, &mut tracker);
                            let child = place.child(&key, &shown, clearances);
                            flags.record(child.shown(), &tracker, class.derived);
                            (shown, self.node(value, &child, clearances, flags))
                        })
                        .collect(),
                )
            }
            scalar => {
                let mut tracker = Tracker::default();
                let (value, derived) = self.scalar(scalar, place, &mut tracker);
                flags.record(place.shown(), &tracker, derived);
                value
            }
        }
    }

    fn scalar(&self, node: Node, place: &Place, tracker: &mut Tracker) -> (Node, bool) {
        match node {
            Node::Int(value) => {
                let (class, _) = place.class("long");
                let value = match self.number_pseudonym(&value.to_string(), &class, tracker) {
                    Some(pseudonym) => pseudonym.parse().map_or(Node::Text(pseudonym), Node::Int),
                    None => Node::Int(value),
                };
                (value, class.derived)
            }
            Node::Double(value)
                if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e18 =>
            {
                let (class, _) = place.class("double");
                let digits = format!("{}", value as i64);
                let value = match self.number_pseudonym(&digits, &class, tracker) {
                    Some(pseudonym) => pseudonym
                        .parse::<i64>()
                        .map_or(Node::Text(pseudonym), |number| Node::Double(number as f64)),
                    None => Node::Double(value),
                };
                (value, class.derived)
            }
            Node::Decimal(text) => {
                let (class, _) = place.class("decimal");
                let value = match self.number_pseudonym(&text, &class, tracker) {
                    Some(pseudonym) if pseudonym.parse::<i64>().is_ok() => Node::Decimal(pseudonym),
                    Some(pseudonym) => Node::Text(pseudonym),
                    None => Node::Decimal(text),
                };
                (value, class.derived)
            }
            Node::Text(text) => {
                let (class, tainted) = place.class(DOCUMENT_TEXT);
                (
                    Node::Text(self.text(&text, &class, tainted, tracker)),
                    class.derived,
                )
            }
            Node::Date(text) => {
                let (class, _) = place.class("date");
                let value = if class.derived && class.forced_kind.is_none() {
                    self.pseudonymize(Kind::BirthDate, &text, Reason::Derived, tracker)
                } else if class.forced_kind == Some(Kind::BirthDate) {
                    self.pseudonymize(Kind::BirthDate, &text, forced_reason(&class), tracker)
                } else {
                    text
                };
                (Node::Date(value), class.derived)
            }
            Node::Binary { len, .. } => self.opaque_node("binary", len, place, tracker),
            Node::Opaque { type_name, len } => self.opaque_node(&type_name, len, place, tracker),
            Node::Double(_)
            | Node::Null
            | Node::Bool(_)
            | Node::ObjectId(_)
            | Node::Uuid(_)
            | Node::Timestamp(_)
            | Node::MinKey
            | Node::MaxKey
            | Node::Array(_)
            | Node::Object(_) => (node, false),
        }
    }

    fn opaque_node(
        &self,
        type_name: &str,
        len: usize,
        place: &Place,
        tracker: &mut Tracker,
    ) -> (Node, bool) {
        tracker.note(Kind::PersonName, Reason::Default, false);
        let (class, _) = place.class(type_name);
        (
            Node::Text(format!("<{type_name} len={len}>")),
            class.derived,
        )
    }

    pub fn free_text(&self, text: &str) -> String {
        let mut tracker = Tracker::default();
        let scrubbed = self.scrub_injected(text, false, &mut tracker);
        self.replace_spans(&scrubbed, &detect_spans(&scrubbed), false, &mut tracker)
    }

    pub fn message(&self, message: &DatabaseMessage) -> DatabaseMessage {
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
        DatabaseMessage {
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

    fn cell(&self, cell: Cell, class: &ColumnClass, tainted: bool, tracker: &mut Tracker) -> Value {
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
            Cell::Text(text) => Value::String(self.text(&text, class, tainted, tracker)),
            Cell::Json(value) => self.json(value, None, class.structural_only, tainted, tracker),
            Cell::Uuid(text) | Cell::Time(text) | Cell::Interval(text) => Value::String(text),
            Cell::Date(text) if class.derived && class.forced_kind.is_none() => {
                Value::String(self.pseudonymize(Kind::BirthDate, &text, Reason::Derived, tracker))
            }
            Cell::Date(text) | Cell::Timestamp(text) | Cell::TimestampTz(text) => {
                if class.forced_kind == Some(Kind::BirthDate) {
                    Value::String(self.pseudonymize(
                        Kind::BirthDate,
                        &text,
                        forced_reason(class),
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
                    .map(|item| self.cell(item, class, tainted, tracker))
                    .collect(),
            ),
            Cell::Composite(fields) => {
                let mut object = Map::new();
                for (name, field) in fields {
                    let field_class = classify_column(&ColumnInfo {
                        column_name: (!class.structural_only).then_some(name.as_str()),
                        type_name: field_type(&field),
                        derived: class.derived,
                    });
                    let field_class = ColumnClass {
                        structural_only: class.structural_only,
                        ..field_class
                    };
                    object.insert(name, self.cell(field, &field_class, tainted, tracker));
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
        let Some(pseudonym) = self.number_pseudonym(digits, class, tracker) else {
            return original;
        };
        if original.is_number()
            && let Ok(number) = pseudonym.parse::<i64>()
        {
            return json!(number);
        }
        Value::String(pseudonym)
    }

    fn number_pseudonym(
        &self,
        digits: &str,
        class: &ColumnClass,
        tracker: &mut Tracker,
    ) -> Option<String> {
        let hint = match class.forced_kind {
            Some(hint) => hint,
            None if class.derived => DERIVED_NUMBER_KINDS
                .iter()
                .find_map(|kind| numeric_shape_kind(digits, Some(*kind)))?,
            None => return None,
        };
        let kind = numeric_shape_kind(digits, Some(hint)).unwrap_or(hint);
        let integral = digits.split('.').next().unwrap_or(digits);
        Some(self.pseudonymize(kind, integral, forced_reason(class), tracker))
    }

    fn text(
        &self,
        text: &str,
        class: &ColumnClass,
        tainted: bool,
        tracker: &mut Tracker,
    ) -> String {
        let scrubbed = self.scrub_injected(text, tainted, tracker);
        if self.trusts(scrubbed.trim(), tainted) {
            return scrubbed;
        }
        let verdict = if self.is_enum_label(&scrubbed, class) {
            TextVerdict::Clear
        } else {
            judge_text(&scrubbed, class)
        };
        match verdict {
            TextVerdict::Clear if tainted && residue_has_words(&scrubbed, &[]) => {
                self.opaque_derived(&scrubbed, tracker)
            }
            TextVerdict::Spans(spans) if tainted && residue_has_words(&scrubbed, &spans) => {
                self.opaque_derived(&scrubbed, tracker)
            }
            TextVerdict::Clear => scrubbed,
            TextVerdict::Spans(spans) if class.derived && leaves_words(&scrubbed, &spans) => {
                self.pseudonymize(spans[0].kind, &scrubbed, Reason::Derived, tracker)
            }
            TextVerdict::Spans(spans) => self.replace_spans(&scrubbed, &spans, tainted, tracker),
            TextVerdict::WholeValue { kind, reason } => {
                self.pseudonymize(kind, &scrubbed, reason, tracker)
            }
            TextVerdict::Digest => {
                tracker.note(Kind::PersonName, Reason::Derived, false);
                self.pseudonymizer.opaque_token("digest", &scrubbed)
            }
        }
    }

    fn cleared(
        &self,
        index: usize,
        column: &ColumnMeta,
        class: ColumnClass,
        tainted: bool,
    ) -> ColumnClass {
        let unnamed_derived = class.derived && class.forced_kind.is_none();
        match self.clearances.get(index) {
            Some(Clearance::ProvenClear) if unnamed_derived && !tainted => ColumnClass {
                forced_kind: None,
                clear_by_type: true,
                derived: false,
                structural_only: false,
                forced_by_policy: false,
            },
            Some(Clearance::ClearByPolicy) if unnamed_derived => ColumnClass {
                forced_kind: None,
                clear_by_type: false,
                derived: false,
                structural_only: true,
                forced_by_policy: false,
            },
            Some(Clearance::ClearByPolicy) if !class.derived => ColumnClass {
                structural_only: true,
                ..classify_column(&ColumnInfo {
                    column_name: None,
                    type_name: &column.type_name,
                    derived: false,
                })
            },
            Some(Clearance::PersonalByPolicy(kind)) if !class.derived => ColumnClass {
                forced_kind: Some(*kind),
                clear_by_type: false,
                derived: false,
                structural_only: false,
                forced_by_policy: true,
            },
            _ => class,
        }
    }

    fn is_enum_label(&self, text: &str, class: &ColumnClass) -> bool {
        class.forced_kind.is_none()
            && !self.enum_labels.is_empty()
            && self.enum_labels.contains(&fold(text.trim()))
    }

    fn opaque_derived(&self, text: &str, tracker: &mut Tracker) -> String {
        tracker.note_label(DERIVED_PERSONAL, Reason::Derived);
        self.pseudonymizer.opaque_token("derived", text)
    }

    fn json(
        &self,
        value: Value,
        key_kind: Option<Kind>,
        structural_only: bool,
        tainted: bool,
        tracker: &mut Tracker,
    ) -> Value {
        match value {
            Value::String(text) => {
                let class = ColumnClass {
                    forced_kind: key_kind,
                    clear_by_type: false,
                    derived: false,
                    structural_only,
                    forced_by_policy: false,
                };
                Value::String(self.text(&text, &class, tainted, tracker))
            }
            Value::Number(number) => match key_kind {
                Some(kind) => {
                    let class = ColumnClass {
                        forced_kind: Some(kind),
                        clear_by_type: false,
                        derived: false,
                        structural_only: false,
                        forced_by_policy: false,
                    };
                    let text = number.to_string();
                    self.number(&text, Value::Number(number), &class, tracker)
                }
                None => Value::Number(number),
            },
            Value::Array(items) => Value::Array(
                items
                    .into_iter()
                    .map(|item| self.json(item, key_kind, structural_only, tainted, tracker))
                    .collect(),
            ),
            Value::Object(entries) => Value::Object(
                entries
                    .into_iter()
                    .map(|(key, item)| {
                        let kind = if structural_only {
                            None
                        } else {
                            dictionary_kind(&key)
                        };
                        let key = self.json_key(&key, structural_only, tainted, tracker);
                        let item = self.json(item, kind, structural_only, tainted, tracker);
                        (key, item)
                    })
                    .collect(),
            ),
            other => other,
        }
    }

    fn json_key(
        &self,
        key: &str,
        structural_only: bool,
        tainted: bool,
        tracker: &mut Tracker,
    ) -> String {
        let scrubbed = self.scrub_injected(key, tainted, tracker);
        let spans = if structural_only {
            structural_spans(&scrubbed)
        } else {
            detect_spans(&scrubbed)
        };
        if tainted && !self.trusts(scrubbed.trim(), tainted) && residue_has_words(&scrubbed, &spans)
        {
            return self.opaque_derived(&scrubbed, tracker);
        }
        self.replace_spans(&scrubbed, &spans, tainted, tracker)
    }

    fn replace_spans(
        &self,
        text: &str,
        spans: &[Span],
        tainted: bool,
        tracker: &mut Tracker,
    ) -> String {
        let mut output = text.to_string();
        let mut ordered = spans.to_vec();
        ordered.sort_by_key(|span| std::cmp::Reverse(span.start));
        for span in ordered {
            let Some(fragment) = text.get(span.start..span.end) else {
                continue;
            };
            if self.trusts(fragment, tainted) {
                continue;
            }
            let pseudonym = self.pseudonymize(span.kind, fragment, Reason::Detected, tracker);
            output.replace_range(span.start..span.end, &pseudonym);
        }
        output
    }

    fn is_pseudonym(&self, text: &str) -> bool {
        self.is_injected_pseudonym(text) || self.pseudonymizer.looks_like_pseudonym(text).is_some()
    }

    fn is_known_pseudonym(&self, text: &str) -> bool {
        self.is_injected_pseudonym(text) || self.pseudonymizer.reverse(text).is_some()
    }

    fn is_injected_pseudonym(&self, text: &str) -> bool {
        self.injected.iter().any(|(_, pseudonym)| pseudonym == text)
    }

    fn trusts(&self, text: &str, tainted: bool) -> bool {
        if tainted {
            self.is_known_pseudonym(text)
        } else {
            self.is_pseudonym(text)
        }
    }

    fn scrub_injected(&self, text: &str, tainted: bool, tracker: &mut Tracker) -> String {
        let mut output = text.to_string();
        for (real, pseudonym) in &self.injected {
            let embedded = |token: &str| {
                if real.chars().count() < MIN_EMBEDDED_INJECTED_CHARS || self.trusts(token, tainted)
                {
                    None
                } else {
                    Some(self.pseudonymizer.opaque_token("value", token))
                }
            };
            if let Some(replaced) = replace_injected(&output, real, pseudonym, embedded) {
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

const DOCUMENT_TEXT: &str = "string";

const DERIVED_PERSONAL: &str = "derived_personal";

const MIN_RESIDUE_WORD_CHARS: usize = 3;

const MIN_EMBEDDED_INJECTED_CHARS: usize = 4;

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

fn tainted_positions(lineage: Option<&[Lineage]>, count: usize) -> Vec<bool> {
    let Some(items) = lineage else {
        return vec![false; count];
    };
    let known = items
        .iter()
        .position(|item| *item == Lineage::Unknown)
        .unwrap_or(items.len());
    if known > count || (known == items.len() && known != count) {
        return vec![true; count];
    }
    (0..count)
        .map(|index| index >= known || items[index] == Lineage::Personal)
        .collect()
}

fn leaves_words(text: &str, spans: &[Span]) -> bool {
    text.split_whitespace().count() <= DERIVED_WHOLE_VALUE_WORDS && residue_has_words(text, spans)
}

fn residue_has_words(text: &str, spans: &[Span]) -> bool {
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
        .any(|word| word.chars().count() >= MIN_RESIDUE_WORD_CHARS)
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

#[cfg(test)]
mod integration {
    use super::*;
    use crate::contexts::anonymization::domain::types::Lineage::{Clean, Personal, Unknown};
    use crate::contexts::anonymization::domain::types::{ColumnSource, GeneratorConfig};
    use crate::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;

    fn pseudonymizer() -> (tempfile::TempDir, Pseudonymizer) {
        let dir = tempfile::tempdir().expect("data dir");
        let pseudonymizer =
            open_pseudonymizer(dir.path(), GeneratorConfig::default()).expect("pseudonymizer");
        (dir, pseudonymizer)
    }

    fn text_column(name: &str, source: Option<&str>) -> ColumnMeta {
        ColumnMeta {
            name: name.to_string(),
            type_name: "text".to_string(),
            source: source.map(|column| ColumnSource {
                schema: "public".to_string(),
                table: "customers".to_string(),
                column: column.to_string(),
            }),
        }
    }

    fn message(sqlstate: &str, text: &str, detail: Option<&str>) -> DatabaseMessage {
        DatabaseMessage {
            severity: "ERROR".to_string(),
            sqlstate: Some(sqlstate.to_string()),
            message: text.to_string(),
            detail: detail.map(str::to_string),
            hint: None,
            where_: Some("SQL function \"f\" statement 1".to_string()),
        }
    }

    #[test]
    fn un_nom_injecte_court_laisse_intacts_les_mots_plus_longs_qui_le_contiennent() {
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
        let anonymized = anonymizer.rows(&columns, rows, None);
        let expected = baseline.rows(&columns, vec![first()], None);
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
    fn une_valeur_injectee_noyee_dans_un_jeton_plus_long_rend_tout_le_jeton_opaque() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let pseudonym = pseudonymizer.pseudonymize(Kind::LastName, "Kervadec").text;
        let injected = [
            ("Kervadec".to_string(), pseudonym.clone()),
            ("Roy".to_string(), "Zabulon".to_string()),
        ];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("label", None),
            text_column("code", None),
            text_column("commentaire", Some("commentaire")),
            text_column("ville", Some("ville")),
        ];
        let rows = vec![vec![
            Cell::Text("Kervadec2026".to_string()),
            Cell::Text("xKERVADECx".to_string()),
            Cell::Text("Client Kervadec, appel".to_string()),
            Cell::Text("Troyes".to_string()),
        ]];
        let anonymized = anonymizer.rows(&columns, rows, None);
        let expected_troyes =
            baseline.rows(&columns[3..], vec![vec![Cell::Text("Troyes".into())]], None);
        assert_eq!(
            anonymized.rows[0][0],
            json!(pseudonymizer.opaque_token("value", "Kervadec2026"))
        );
        assert_eq!(
            anonymized.rows[0][1],
            json!(pseudonymizer.opaque_token("value", "xKERVADECx"))
        );
        assert_eq!(
            anonymized.rows[0][2],
            json!(format!("Client {pseudonym}, appel"))
        );
        assert_eq!(anonymized.rows[0][3], expected_troyes.rows[0][0]);
        assert!(anonymized.per_column[0].is_some());
        assert!(anonymized.per_column[1].is_some());

        let free = anonymizer.free_text("réf Kervadec2026 pour Kervadec à Troyes");
        assert_eq!(
            free,
            format!(
                "réf {} pour {pseudonym} à Troyes",
                pseudonymizer.opaque_token("value", "Kervadec2026")
            )
        );
    }

    #[test]
    fn une_valeur_injectee_dans_un_pseudonyme_connu_est_laissee_telle_quelle() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let injected = [
            ("Kervadec".to_string(), "Zabulon".to_string()),
            ("Odriganvanec".to_string(), "Kervadecson".to_string()),
        ];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);
        assert_eq!(anonymizer.free_text("vu Odriganvanec"), "vu Kervadecson");
    }

    #[test]
    fn des_colonnes_homonymes_ont_chacune_leur_propre_indicateur() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("email", Some("email")),
            text_column("status", Some("status")),
            text_column("email", Some("email")),
        ];
        let rows = vec![vec![
            Cell::Text("ylwenna.kervadec@canary-mail.fr".to_string()),
            Cell::Text("PAID".to_string()),
            Cell::Text("odrigan.hazvekmor@canary-mail.fr".to_string()),
        ]];
        let anonymized = anonymizer.rows(&columns, rows, None);
        assert_eq!(anonymized.per_column.len(), 3);
        assert!(anonymized.per_column[0].is_some());
        assert!(anonymized.per_column[1].is_none());
        assert!(anonymized.per_column[2].is_some());
        assert_eq!(anonymized.pseudonymized.len(), 1);
    }

    #[test]
    fn les_messages_d_entree_invalide_citent_des_jetons_opaques_et_non_les_valeurs() {
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
            .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr")
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
    fn le_detail_d_une_violation_d_unicite_masque_la_cle_sauf_si_c_est_un_pseudonyme() {
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
            .pseudonymize(Kind::Email, "odrigan.hazvekmor@canary-mail.fr")
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

    #[test]
    fn une_valeur_derivee_de_lignee_personnelle_devient_opaque_si_aucun_detecteur_ne_la_reconnait()
    {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let mut length = text_column("length", None);
        length.type_name = "int4".to_string();
        let columns = [
            text_column("split_part", None),
            text_column("lower", None),
            text_column("initials", None),
            text_column("line", None),
            length,
            text_column("label", None),
        ];
        let row = || {
            vec![
                Cell::Text("ylwenna.kervadec".to_string()),
                Cell::Text("ylwenna.kervadec@canary-mail.fr".to_string()),
                Cell::Text("YK".to_string()),
                Cell::Text("ylwenna.kervadec au 06 41 27 83 95".to_string()),
                Cell::Int(31),
                Cell::Text("PAID en_cours".to_string()),
            ]
        };
        let lineage = [Personal, Personal, Personal, Personal, Personal, Clean];
        let anonymized = anonymizer.rows(&columns, vec![row()], Some(&lineage));
        assert_eq!(
            anonymized.rows[0][0],
            json!(pseudonymizer.opaque_token("derived", "ylwenna.kervadec"))
        );
        let email = anonymized.rows[0][1].as_str().expect("text");
        assert!(email.contains('@'), "{email}");
        assert!(!email.contains("kervadec"), "{email}");
        assert_eq!(anonymized.rows[0][2], json!("YK"));
        assert_eq!(
            anonymized.rows[0][3],
            json!(pseudonymizer.opaque_token("derived", "ylwenna.kervadec au 06 41 27 83 95"))
        );
        assert_eq!(anonymized.rows[0][4], json!(31));
        assert_eq!(anonymized.rows[0][5], json!("PAID en_cours"));
        assert_eq!(
            anonymized.per_column[0],
            Some(ColumnFlag {
                kind: "derived_personal".to_string(),
                why: Reason::Derived,
            })
        );
        assert!(anonymized.per_column[2].is_none());
        assert!(anonymized.per_column[5].is_none());
    }

    #[test]
    fn une_valeur_souillee_imbriquee_dans_du_json_ou_un_tableau_devient_opaque() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("jsonb_build_object", None),
            text_column("array_agg", None),
        ];
        let rows = vec![vec![
            Cell::Json(json!({"ylwenna.kervadec": "ylwenna.kervadec", "n": 2})),
            Cell::Array(vec![Cell::Text("ylwenna.kervadec".to_string())]),
        ]];
        let anonymized = anonymizer.rows(&columns, rows, Some(&[Personal, Personal]));
        let token = pseudonymizer.opaque_token("derived", "ylwenna.kervadec");
        assert_eq!(
            anonymized.rows[0][0],
            json!({ token.clone(): token.clone(), "n": 2 })
        );
        assert_eq!(anonymized.rows[0][1], json!([token]));
    }

    #[test]
    fn toute_colonne_derivee_a_partir_d_une_position_inconnue_est_souillee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("upper", None),
            text_column("status", Some("status")),
            text_column("split_part", None),
            text_column("label", None),
        ];
        let rows = vec![vec![
            Cell::Text("PAID en_cours".to_string()),
            Cell::Text("PAID en_cours".to_string()),
            Cell::Text("ylwenna.kervadec".to_string()),
            Cell::Text("PAID en_cours".to_string()),
        ]];
        let anonymized = anonymizer.rows(&columns, rows, Some(&[Clean, Unknown]));
        assert_eq!(anonymized.rows[0][0], json!("PAID en_cours"));
        assert_eq!(anonymized.rows[0][1], json!("PAID en_cours"));
        assert_eq!(
            anonymized.rows[0][2],
            json!(pseudonymizer.opaque_token("derived", "ylwenna.kervadec"))
        );
        assert_eq!(
            anonymized.rows[0][3],
            json!(pseudonymizer.opaque_token("derived", "PAID en_cours"))
        );
        assert!(anonymized.per_column[0].is_none());
        assert!(anonymized.per_column[1].is_none());
    }

    #[test]
    fn des_elements_connus_au_dela_des_colonnes_souillent_toute_colonne_derivee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [text_column("upper", None)];
        let rows = vec![vec![Cell::Text("PAID en_cours".to_string())]];
        let anonymized = anonymizer.rows(&columns, rows, Some(&[Clean, Clean, Unknown]));
        assert_eq!(
            anonymized.rows[0][0],
            json!(pseudonymizer.opaque_token("derived", "PAID en_cours"))
        );
    }

    #[test]
    fn une_valeur_souillee_en_forme_de_pseudonyme_n_est_crue_que_si_elle_est_connue() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let known = pseudonymizer
            .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr")
            .text;
        let injected = [(
            "odrigan.hazvekmor@canary-mail.fr".to_string(),
            "paul.rivet@example.org".to_string(),
        )];
        let anonymizer = Anonymizer::new(&pseudonymizer, &injected);
        let columns = [
            text_column("left", None),
            text_column("concat", None),
            text_column("jsonb_build_object", None),
            text_column("known", None),
            text_column("injected", None),
        ];
        let rows = vec![vec![
            Cell::Text("ylwenn@example.com".to_string()),
            Cell::Text("ylwenna.kervadec @example.com".to_string()),
            Cell::Json(json!({"ylwenna.kervadec@example.com": 1})),
            Cell::Text(known.clone()),
            Cell::Text("paul.rivet@example.org".to_string()),
        ]];
        let anonymized = anonymizer.rows(&columns, rows, Some(&[Personal; 5]));
        let rendered = serde_json::to_string(&anonymized.rows).expect("json");
        assert!(!rendered.contains("ylwenn"), "{rendered}");
        assert!(!rendered.contains("kervadec"), "{rendered}");
        assert_eq!(anonymized.rows[0][3], json!(known));
        assert_eq!(anonymized.rows[0][4], json!("paul.rivet@example.org"));
    }

    #[test]
    fn sans_lignee_une_valeur_derivee_garde_le_jugement_valeur_par_valeur() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [text_column("split_part", None), text_column("label", None)];
        let row = || {
            vec![
                Cell::Text("ylwenna.kervadec".to_string()),
                Cell::Text("PAID en_cours".to_string()),
            ]
        };
        let unknown = anonymizer.rows(&columns, vec![row()], None);
        let untainted = anonymizer.rows(&columns, vec![row()], Some(&[Clean, Clean]));
        assert_eq!(unknown.rows, untainted.rows);
        assert_eq!(unknown.per_column, untainted.per_column);
    }

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn single_cell(anonymizer: &Anonymizer, column: ColumnMeta, value: &str) -> AnonymizedRows {
        anonymizer.rows(&[column], vec![vec![Cell::Text(value.to_string())]], None)
    }

    #[test]
    fn un_libelle_d_enum_converti_en_texte_reste_en_clair() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&labels(&["AUTHORIZED"]));
        let anonymized = single_cell(&anonymizer, text_column("status", None), "AUTHORIZED");
        assert_eq!(anonymized.rows[0][0], json!("AUTHORIZED"));
        assert!(anonymized.per_column[0].is_none());
    }

    #[test]
    fn un_libelle_d_enum_reste_en_clair_quelle_que_soit_sa_casse() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&labels(&["AUTHORIZED"]));
        for value in ["authorized", "Authorized", " Authorized "] {
            let anonymized = single_cell(&anonymizer, text_column("lower", None), value);
            assert_eq!(anonymized.rows[0][0], json!(value));
            assert!(anonymized.per_column[0].is_none(), "{value}");
        }
    }

    #[test]
    fn un_libelle_d_enum_en_colonne_inconnue_echappe_a_l_heuristique_de_nom() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&labels(&["SEPA"]));
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let column = || text_column("moyen", Some("moyen"));
        assert!(single_cell(&baseline, column(), "Sepa").per_column[0].is_some());
        let anonymized = single_cell(&anonymizer, column(), "Sepa");
        assert_eq!(anonymized.rows[0][0], json!("Sepa"));
        assert!(anonymized.per_column[0].is_none());
    }

    #[test]
    fn un_prenom_derive_absent_des_libelles_d_enum_reste_pseudonymise() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&labels(&["AUTHORIZED", "CASH"]));
        let anonymized = single_cell(&anonymizer, text_column("initcap", None), "Marie");
        assert_ne!(anonymized.rows[0][0], json!("Marie"));
        assert!(anonymized.per_column[0].is_some());
    }

    #[test]
    fn une_colonne_nom_egale_a_un_libelle_d_enum_reste_pseudonymisee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&labels(&["MARTIN"]));
        let anonymized = single_cell(&anonymizer, text_column("nom", Some("nom")), "Martin");
        assert_ne!(anonymized.rows[0][0], json!("Martin"));
        assert_eq!(
            anonymized.per_column[0],
            Some(ColumnFlag {
                kind: "last_name".to_string(),
                why: Reason::Dictionary,
            })
        );
    }

    #[test]
    fn un_libelle_d_enum_dans_une_colonne_souillee_reste_opaque() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&labels(&["KERVADEC"]));
        let anonymized = anonymizer.rows(
            &[text_column("lower", None)],
            vec![vec![Cell::Text("kervadec".to_string())]],
            Some(&[Personal]),
        );
        assert_ne!(anonymized.rows[0][0], json!("kervadec"));
        assert!(anonymized.per_column[0].is_some());
    }

    #[test]
    fn sans_libelle_d_enum_le_jugement_est_inchange() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]).with_enum_labels(&[]);
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("status", None),
            text_column("moyen", Some("moyen")),
        ];
        let row = || {
            vec![
                Cell::Text("AUTHORIZED".to_string()),
                Cell::Text("Sepa".to_string()),
            ]
        };
        let anonymized = anonymizer.rows(&columns, vec![row()], None);
        let expected = baseline.rows(&columns, vec![row()], None);
        assert_eq!(anonymized.rows, expected.rows);
        assert_eq!(anonymized.per_column, expected.per_column);
        assert_ne!(anonymized.rows[0][0], json!("AUTHORIZED"));
    }

    fn typed_column(name: &str, type_name: &str) -> ColumnMeta {
        ColumnMeta {
            name: name.to_string(),
            type_name: type_name.to_string(),
            source: None,
        }
    }

    fn derived_row() -> (Vec<ColumnMeta>, Vec<Vec<Cell>>) {
        (
            vec![
                text_column("concat", None),
                typed_column("placed_at", "date"),
                typed_column("count", "int8"),
            ],
            vec![vec![
                Cell::Text("Martin".to_string()),
                Cell::Date("1985-11-02".to_string()),
                Cell::Int(33641278395),
            ]],
        )
    }

    #[test]
    fn une_colonne_derivee_prouvee_en_clair_garde_ses_valeurs() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let (columns, rows) = derived_row();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ProvenClear; 3]);
        let anonymized = anonymizer.rows(&columns, rows, None);
        assert_eq!(
            anonymized.rows[0],
            vec![json!("Martin"), json!("1985-11-02"), json!(33641278395_i64)]
        );
        assert_eq!(anonymized.per_column, vec![None, None, None]);
    }

    #[test]
    fn une_colonne_derivee_non_prouvee_reste_jugee_comme_avant() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let (columns, rows) = derived_row();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::Unproven; 3]);
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let anonymized = anonymizer.rows(&columns, rows.clone(), None);
        let expected = baseline.rows(&columns, rows, None);
        assert_eq!(anonymized.rows, expected.rows);
        assert!(anonymized.per_column.iter().all(Option::is_some));
    }

    #[test]
    fn une_colonne_prouvee_en_clair_mais_nommee_comme_une_donnee_personnelle_reste_pseudonymisee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ProvenClear]);
        let anonymized = single_cell(&anonymizer, text_column("nom", None), "Martin");
        assert_ne!(anonymized.rows[0][0], json!("Martin"));
        assert!(anonymized.per_column[0].is_some());
    }

    #[test]
    fn une_colonne_prouvee_en_clair_mais_souillee_reste_opaque() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ProvenClear]);
        let anonymized = anonymizer.rows(
            &[text_column("concat", None)],
            vec![vec![Cell::Text("paid kervadec".to_string())]],
            Some(&[Personal]),
        );
        assert_ne!(anonymized.rows[0][0], json!("paid kervadec"));
        assert!(anonymized.per_column[0].is_some());
    }

    #[test]
    fn une_preuve_de_clarte_sur_une_colonne_lue_directement_est_ignoree() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ProvenClear]);
        let anonymized = single_cell(&anonymizer, text_column("nom", Some("nom")), "Martin");
        assert_ne!(anonymized.rows[0][0], json!("Martin"));
    }

    #[test]
    fn un_pseudonyme_ecrit_en_litteral_dans_une_colonne_prouvee_revient_tel_quel() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let injected = [(
            "ylwenna.kervadec@canary-mail.fr".to_string(),
            "claire.martin@example.net".to_string(),
        )];
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &injected).with_clearances(&[Clearance::ProvenClear]);
        let value = "paid claire.martin@example.net";
        let anonymized = single_cell(&anonymizer, text_column("etiquette", None), value);
        assert_eq!(anonymized.rows[0][0], json!(value));
        assert!(anonymized.per_column[0].is_none());
    }

    #[test]
    fn une_valeur_reelle_traduite_reste_masquee_meme_dans_une_colonne_prouvee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let injected = [(
            "ylwenna.kervadec@canary-mail.fr".to_string(),
            "claire.martin@example.net".to_string(),
        )];
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &injected).with_clearances(&[Clearance::ProvenClear]);
        let anonymized = single_cell(
            &anonymizer,
            text_column("etiquette", None),
            "paid ylwenna.kervadec@canary-mail.fr",
        );
        assert_eq!(
            anonymized.rows[0][0],
            json!("paid claire.martin@example.net")
        );
    }

    #[test]
    fn une_lignee_qui_ne_correspond_pas_aux_colonnes_souille_toute_colonne_derivee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[]);
        let columns = [
            text_column("split_part", None),
            text_column("status", Some("status")),
        ];
        let rows = vec![vec![
            Cell::Text("ylwenna.kervadec".to_string()),
            Cell::Text("PAID en_cours".to_string()),
        ]];
        let anonymized = anonymizer.rows(&columns, rows, Some(&[Clean]));
        assert_eq!(
            anonymized.rows[0][0],
            json!(pseudonymizer.opaque_token("derived", "ylwenna.kervadec"))
        );
        assert_eq!(anonymized.rows[0][1], json!("PAID en_cours"));
    }
    fn source_column(name: &str, type_name: &str) -> ColumnMeta {
        ColumnMeta {
            name: name.to_string(),
            type_name: type_name.to_string(),
            source: Some(ColumnSource {
                schema: "public".to_string(),
                table: "products".to_string(),
                column: name.to_string(),
            }),
        }
    }

    #[test]
    fn une_colonne_declaree_en_clair_garde_sa_valeur_malgre_le_dictionnaire() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let column = || source_column("product_name", "text");
        assert_eq!(
            single_cell(&baseline, column(), "Garance Bleue").per_column[0]
                .as_ref()
                .map(|flag| flag.why),
            Some(Reason::Dictionary)
        );
        let anonymized = single_cell(&anonymizer, column(), "Garance Bleue");
        assert_eq!(anonymized.rows[0][0], json!("Garance Bleue"));
        assert!(anonymized.per_column[0].is_none());
    }

    #[test]
    fn une_colonne_declaree_en_clair_echappe_a_l_heuristique_de_nom() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let column = || source_column("libelle", "text");
        assert!(single_cell(&baseline, column(), "Ylwenna Kervadec").per_column[0].is_some());
        let anonymized = single_cell(&anonymizer, column(), "Ylwenna Kervadec");
        assert_eq!(anonymized.rows[0][0], json!("Ylwenna Kervadec"));
        assert!(anonymized.per_column[0].is_none());
    }

    #[test]
    fn une_colonne_declaree_en_clair_remplace_quand_meme_une_adresse_e_mail() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let anonymized = single_cell(
            &anonymizer,
            source_column("product_name", "text"),
            "Garance ylwenna.kervadec@canary-mail.fr",
        );
        let value = anonymized.rows[0][0].as_str().expect("text");
        assert!(value.starts_with("Garance "), "{value}");
        assert!(!value.contains("canary-mail"), "{value}");
        assert_eq!(
            anonymized.per_column[0],
            Some(ColumnFlag {
                kind: "email".to_string(),
                why: Reason::Detected,
            })
        );
    }

    #[test]
    fn une_colonne_declaree_en_clair_remplace_quand_meme_un_telephone_et_un_iban() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let value = "appel 06 41 27 83 95 iban FR76 3000 4000 0312 3456 7890 143";
        let anonymized = single_cell(&anonymizer, source_column("reference", "text"), value);
        let text = anonymized.rows[0][0].as_str().expect("text");
        assert!(!text.contains("06 41 27 83 95"), "{text}");
        assert!(!text.contains("FR76 3000"), "{text}");
        assert!(text.starts_with("appel "), "{text}");
    }

    #[test]
    fn le_json_d_une_colonne_declaree_en_clair_ne_garde_que_le_filet_structurel() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let anonymized = anonymizer.rows(
            &[source_column("details", "jsonb")],
            vec![vec![Cell::Json(json!({
                "nom": "Garance",
                "mail": "ylwenna.kervadec@canary-mail.fr"
            }))]],
            None,
        );
        assert_eq!(anonymized.rows[0][0]["nom"], json!("Garance"));
        let mail = anonymized.rows[0][0]["mail"].as_str().expect("text");
        assert!(!mail.contains("canary-mail"), "{mail}");
    }

    #[test]
    fn une_colonne_declaree_personnelle_est_pseudonymisee_avec_le_type_donne() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let baseline = Anonymizer::new(&pseudonymizer, &[]);
        let anonymizer = Anonymizer::new(&pseudonymizer, &[])
            .with_clearances(&[Clearance::PersonalByPolicy(Kind::LastName)]);
        let column = || source_column("code_lot", "text");
        assert!(single_cell(&baseline, column(), "lot-42").per_column[0].is_none());
        let anonymized = single_cell(&anonymizer, column(), "lot-42");
        assert_ne!(anonymized.rows[0][0], json!("lot-42"));
        assert_eq!(
            anonymized.per_column[0],
            Some(ColumnFlag {
                kind: "last_name".to_string(),
                why: Reason::Policy,
            })
        );
    }

    #[test]
    fn une_colonne_numerique_declaree_personnelle_est_pseudonymisee() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer = Anonymizer::new(&pseudonymizer, &[])
            .with_clearances(&[Clearance::PersonalByPolicy(Kind::Siren)]);
        let anonymized = anonymizer.rows(
            &[source_column("numero", "int8")],
            vec![vec![Cell::Int(418273652)]],
            None,
        );
        assert_ne!(anonymized.rows[0][0], json!(418273652));
        assert_eq!(
            anonymized.per_column[0],
            Some(ColumnFlag {
                kind: "siren".to_string(),
                why: Reason::Policy,
            })
        );
    }

    #[test]
    fn une_colonne_derivee_d_une_colonne_declaree_en_clair_garde_le_filet_structurel() {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let clear = single_cell(&anonymizer, text_column("v", None), "Garance Bleue paid");
        assert_eq!(clear.rows[0][0], json!("Garance Bleue paid"));
        assert!(clear.per_column[0].is_none());
        let email = single_cell(
            &anonymizer,
            text_column("v", None),
            "Garance ylwenna.kervadec@canary-mail.fr",
        );
        let value = email.rows[0][0].as_str().expect("text");
        assert!(!value.contains("canary-mail"), "{value}");
    }

    #[test]
    fn une_lignee_prouvee_par_la_politique_l_emporte_sur_la_souillure_par_le_nom_mais_garde_le_filet()
     {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let clear = anonymizer.rows(
            &[text_column("concat", None)],
            vec![vec![Cell::Text("Garance Bleue paid".to_string())]],
            Some(&[Personal]),
        );
        assert_eq!(clear.rows[0][0], json!("Garance Bleue paid"));
        let email = anonymizer.rows(
            &[text_column("concat", None)],
            vec![vec![Cell::Text(
                "Garance ylwenna.kervadec@canary-mail.fr".to_string(),
            )]],
            Some(&[Personal]),
        );
        let value = email.rows[0][0].as_str().expect("text");
        assert!(!value.contains("canary-mail"), "{value}");
    }

    #[test]
    fn une_colonne_derivee_nommee_comme_une_donnee_personnelle_reste_pseudonymisee_malgre_la_politique()
     {
        let (_dir, pseudonymizer) = pseudonymizer();
        let anonymizer =
            Anonymizer::new(&pseudonymizer, &[]).with_clearances(&[Clearance::ClearByPolicy]);
        let anonymized = single_cell(&anonymizer, text_column("nom", None), "Martin");
        assert_ne!(anonymized.rows[0][0], json!("Martin"));
    }
}
