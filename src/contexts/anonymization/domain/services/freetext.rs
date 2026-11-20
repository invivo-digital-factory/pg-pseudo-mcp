use std::sync::OnceLock;

use regex::Regex;

use super::detect::{compile, scan};
use super::gazetteer::{name_hit, starts_upper};
use crate::contexts::anonymization::domain::types::{Kind, Span};

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

pub(crate) fn any_case_name_shape(value: &str) -> Option<Kind> {
    let words: Vec<&str> = value.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 || !words.iter().all(|word| is_name_word(word)) {
        return None;
    }
    let letters = |word: &&str| word.chars().filter(|c| c.is_alphabetic()).count();
    if !words.iter().any(|word| letters(word) >= 3) {
        return None;
    }
    Some(if words.len() == 1 {
        Kind::LastName
    } else {
        Kind::PersonName
    })
}

pub(crate) fn is_digest(value: &str) -> bool {
    is_hex(value) || is_bytea_hex(value) || is_base64(value)
}

const MIN_ENCODED_LEN: usize = 8;

fn is_hex(value: &str) -> bool {
    if value.len() < MIN_ENCODED_LEN || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    value.bytes().any(|b| b.is_ascii_alphabetic())
        || hex::decode(value)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .is_some_and(|text| !text.chars().any(char::is_control))
}

fn is_bytea_hex(value: &str) -> bool {
    value.strip_prefix("\\x").is_some_and(|body| {
        !body.is_empty()
            && body.len().is_multiple_of(2)
            && body.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

fn is_base64(value: &str) -> bool {
    use base64::Engine;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE};

    let compact: String = value
        .chars()
        .filter(|c| !matches!(c, '\n' | '\r'))
        .collect();
    if compact.len() < MIN_ENCODED_LEN || is_uuid(&compact) {
        return false;
    }
    let body = compact.trim_end_matches('=');
    let padding = compact.len() - body.len();
    let charset = body
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_'));
    if padding > 2 || body.is_empty() || !charset || !compact.len().is_multiple_of(4) {
        return false;
    }
    if body.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let signal = padding > 0
        || body
            .bytes()
            .any(|b| b.is_ascii_digit() || matches!(b, b'+' | b'/'))
        || (body.bytes().skip(1).any(|b| b.is_ascii_uppercase())
            && body.bytes().any(|b| b.is_ascii_lowercase()));
    signal && (STANDARD.decode(&compact).is_ok() || URL_SAFE.decode(&compact).is_ok())
}

fn is_uuid(value: &str) -> bool {
    let groups: Vec<&str> = value.split('-').collect();
    groups.iter().map(|group| group.len()).eq([8, 4, 4, 4, 12])
        && groups
            .iter()
            .all(|group| group.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn une_adresse_court_jusqu_a_la_ponctuation_suivante() {
        let text = "livraison au 3 bis avenue de la Gare, merci";
        let spans = address_spans(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            &text[spans[0].start..spans[0].end],
            "3 bis avenue de la Gare"
        );
    }

    #[test]
    fn une_civilite_prend_jusqu_a_trois_mots_capitalises() {
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
    fn la_forme_d_un_nom_n_accepte_que_des_mots_capitalises() {
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
    fn les_formes_d_empreinte_sont_reconnues() {
        assert!(is_digest("5d41402abc4b2a76b9719d911017c592"));
        assert!(is_digest(&"a".repeat(40)));
        assert!(is_digest("aGVsbG8gd29ybGQ="));
        assert!(!is_digest("short=="));
        assert!(!is_digest("not base64 at all!"));
    }

    #[test]
    fn un_encodage_hexadecimal_de_toute_longueur_est_une_empreinte() {
        let email = "ylwenna.kervadec@canary-mail.fr";
        assert!(is_digest(&hex::encode(email.as_bytes())));
        assert!(is_digest(&hex::encode(email.as_bytes()).to_uppercase()));
        assert!(is_digest(&hex::encode("Ylwenna".as_bytes())));
        assert!(is_digest(&hex::encode("06 41 27 83 95".as_bytes())));
        assert!(is_digest("7d52d2bbb"));
        assert!(is_digest("5d41402abc4b2a76b9719d911017c59"));
        assert!(is_digest("deadbeef"));
        assert!(is_digest("\\x696c6f6e612e6b"));
        assert!(!is_digest("0641278395"));
        assert!(!is_digest("20261119"));
        assert!(!is_digest("33641278395"));
        assert!(!is_digest("abc123"));
    }

    #[test]
    fn un_court_encodage_base64_est_une_empreinte_mais_pas_un_mot_ordinaire() {
        assert!(is_digest("S2VydmFkZWM="));
        assert!(is_digest("SWxvbmE="));
        assert!(is_digest("T2RpbG9u"));
        assert!(is_digest("aWxvbmF+fn4="));
        assert!(is_digest("aWxvbmF-fn4="));
        for word in [
            "Kervadec",
            "PENDING",
            "REFUNDED",
            "Hazvekmor",
            "12345678",
            "2026-11-19",
            "0b8e4c1a-7d2f-4e5b-9c3a-1f2e3d4c5b6a",
            "en_cours",
        ] {
            assert!(!is_digest(word), "{word}");
        }
    }
}
