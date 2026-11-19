pub mod apply;
mod classify;
mod detect;
mod dictionary;
mod freetext;
mod gazetteer;

use serde::Serialize;

use crate::kind::Kind;

pub use classify::{classify_column, judge_text, numeric_shape_kind};
pub use detect::detect_spans;
pub use dictionary::dictionary_kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Detected,
    Dictionary,
    Default,
    Derived,
}

#[derive(Debug, Clone, Copy)]
pub struct ColumnInfo<'a> {
    pub column_name: Option<&'a str>,
    pub type_name: &'a str,
    pub derived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColumnClass {
    pub forced_kind: Option<Kind>,
    pub clear_by_type: bool,
    pub derived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextVerdict {
    Clear,
    Spans(Vec<Span>),
    WholeValue { kind: Kind, reason: Reason },
    Digest,
}
