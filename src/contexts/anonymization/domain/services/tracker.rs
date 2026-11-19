use crate::contexts::anonymization::domain::types::{Kind, Reason};

#[derive(Default)]
pub struct Tracker {
    pub first: Option<(&'static str, Reason)>,
}

impl Tracker {
    pub fn note_label(&mut self, label: &'static str, reason: Reason) {
        self.first.get_or_insert((label, reason));
    }

    pub fn note(&mut self, kind: Kind, reason: Reason, translated: bool) {
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
