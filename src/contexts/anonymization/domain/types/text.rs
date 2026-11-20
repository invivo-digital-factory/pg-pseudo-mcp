use super::{Kind, Reason};

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
