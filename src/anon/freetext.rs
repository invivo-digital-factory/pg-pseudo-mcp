use std::sync::OnceLock;

use regex::Regex;

use super::Span;
use super::detect::{compile, scan};
use super::gazetteer::{name_hit, starts_upper};
use crate::kind::Kind;

const NAME_PARTICLES: &[&str] = &[
    "d'", "da", "de", "del", "della", "des", "di", "du", "la", "le", "van", "von",
];

struct Patterns {
    address: Regex,
    civility: Regex,
    word: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        address: compile(concat!(
            r"(?i)\b\d{1,4}(?: ?(?:bis|ter|quater|[bt]))?,? +",
            r"(?:(?:rue|avenue|bd|boulevard|chemin|all[ée]e|impasse|place|route|quai|cours|square|r[ée]sidence)\b|av\.)",
            r" +[^,;:.!?()\[\]\n\r]*[^,;:.!?()\[\]\n\r\s]",
        )),
        civility: compile(concat!(
            r"\b(?:M\.|Mme\b\.?|Mlle\b\.?|Mr\b\.?|Mrs\b\.?|Monsieur\b|Madame\b|Mademoiselle\b|Dr\b\.?|Me\b\.?)",
            r" +(\p{Lu}[\p{L}'’-]*(?: +\p{Lu}[\p{L}'’-]*){0,2})",
        )),
        word: compile(r"\p{L}+(?:-\p{L}+)*"),
    })
}

pub(crate) fn address_spans(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    scan(&patterns().address, text, &mut spans, |_, end| {
        Some((end, Kind::Address))
    });
    spans
}

fn name_kind_for(words: usize) -> Kind {
    if words > 1 {
        Kind::PersonName
    } else {
        Kind::LastName
    }
}

fn civility_spans(text: &str) -> Vec<Span> {
    patterns()
        .civility
        .captures_iter(text)
        .filter_map(|captures| captures.get(1))
        .map(|names| Span {
            start: names.start(),
            end: names.end(),
            kind: name_kind_for(names.as_str().split_whitespace().count()),
        })
        .collect()
}

fn gazetteer_spans(text: &str) -> Vec<Span> {
    let words: Vec<(usize, usize, &str)> = patterns()
        .word
        .find_iter(text)
        .map(|found| (found.start(), found.end(), found.as_str()))
        .collect();
    let mut spans = Vec::new();
    for (index, &(start, end, word)) in words.iter().enumerate() {
        let Some(hit) = name_hit(word) else {
            continue;
        };
        let mut span = Span {
            start,
            end,
            kind: hit.kind,
        };
        if hit.first_name && starts_upper(word) {
            for &(next_start, next_end, next_word) in words.iter().skip(index + 1).take(2) {
                let gap = &text[span.end..next_start];
                if gap.is_empty() || !gap.chars().all(|c| c == ' ') || !starts_upper(next_word) {
                    break;
                }
                span.end = next_end;
                span.kind = Kind::PersonName;
            }
        }
        spans.push(span);
    }
    spans
}

pub(crate) fn name_spans(text: &str) -> Vec<Span> {
    let mut spans = civility_spans(text);
    spans.extend(gazetteer_spans(text));
    spans
}

fn is_name_word(word: &str) -> bool {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_alphabetic() {
        return false;
    }
    if word.len() == first.len_utf8() + 1 && word.ends_with('.') {
        return first.is_uppercase();
    }
    word.chars()
        .all(|c| c.is_alphabetic() || matches!(c, '-' | '\'' | '’'))
        && !word.ends_with(['-', '\'', '’'])
}

fn has_lowercase(word: &str) -> bool {
    word.chars().any(char::is_lowercase)
}

pub(crate) fn name_shape(value: &str) -> Option<Kind> {
    let words: Vec<&str> = value.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 || !words.iter().all(|word| is_name_word(word)) {
        return None;
    }
    if let [word] = words.as_slice() {
        let letters = word.chars().filter(|c| c.is_alphabetic()).count();
        return (letters >= 3 && starts_upper(word) && has_lowercase(word))
            .then_some(Kind::LastName);
    }
    let shaped = words.iter().enumerate().all(|(index, word)| {
        starts_upper(word) || (index > 0 && NAME_PARTICLES.contains(&word.to_lowercase().as_str()))
    });
    (shaped && words.iter().any(|word| has_lowercase(word))).then_some(Kind::PersonName)
}

pub(crate) fn is_digest(value: &str) -> bool {
    let hex = matches!(value.len(), 32 | 40 | 64) && value.bytes().all(|b| b.is_ascii_hexdigit());
    hex || is_base64(value)
}

fn is_base64(value: &str) -> bool {
    if value.len() < 16 {
        return false;
    }
    let body = value.trim_end_matches('=');
    let padding = value.len() - body.len();
    padding <= 2
        && !body.is_empty()
        && body
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
        && (value.len().is_multiple_of(4) || padding > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_runs_to_the_next_punctuation() {
        let text = "livraison au 3 bis avenue de la Gare, merci";
        let spans = address_spans(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            &text[spans[0].start..spans[0].end],
            "3 bis avenue de la Gare"
        );
    }

    #[test]
    fn civility_takes_up_to_three_capitalised_words() {
        let text = "voir Mme Marie-Claire Le Kervadec demain";
        let spans = civility_spans(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            &text[spans[0].start..spans[0].end],
            "Marie-Claire Le Kervadec"
        );
        assert_eq!(spans[0].kind, Kind::PersonName);
        assert!(civility_spans("me voici").is_empty());
    }

    #[test]
    fn name_shape_accepts_capitalised_words_only() {
        assert_eq!(name_shape("Kervadec"), Some(Kind::LastName));
        assert_eq!(name_shape("Jean-Luc Le Gall"), Some(Kind::PersonName));
        assert_eq!(name_shape("Jean de La Fontaine"), Some(Kind::PersonName));
        assert_eq!(name_shape("DUPONT Jean"), Some(Kind::PersonName));
        assert_eq!(name_shape("J. Dupont"), Some(Kind::PersonName));
        assert_eq!(name_shape("PAID"), None);
        assert_eq!(name_shape("EN COURS"), None);
        assert_eq!(name_shape("pending"), None);
        assert_eq!(name_shape("en_cours"), None);
        assert_eq!(name_shape("Li"), None);
        assert_eq!(name_shape("En attente"), None);
        assert_eq!(name_shape("Un Deux Trois Quatre Cinq"), None);
    }

    #[test]
    fn digest_shapes() {
        assert!(is_digest("5d41402abc4b2a76b9719d911017c592"));
        assert!(is_digest(&"a".repeat(40)));
        assert!(is_digest("aGVsbG8gd29ybGQ="));
        assert!(!is_digest("5d41402abc4b2a76b9719d911017c59"));
        assert!(!is_digest("short=="));
        assert!(!is_digest("not base64 at all!"));
    }
}
