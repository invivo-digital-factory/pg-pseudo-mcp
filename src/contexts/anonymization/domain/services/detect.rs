use std::net::Ipv6Addr;
use std::sync::OnceLock;

use regex::Regex;

use super::freetext;
use crate::contexts::anonymization::domain::types::{Kind, Span};

const IBAN_LENGTHS: &[(&str, usize)] = &[
    ("AT", 20),
    ("BE", 16),
    ("CH", 21),
    ("DE", 22),
    ("DK", 18),
    ("ES", 24),
    ("FI", 18),
    ("FR", 27),
    ("GB", 22),
    ("IE", 22),
    ("IT", 27),
    ("LU", 20),
    ("MC", 27),
    ("NL", 18),
    ("NO", 15),
    ("PL", 28),
    ("PT", 25),
    ("SE", 24),
];

struct Detectors {
    email: Regex,
    phone: Regex,
    iban: Regex,
    siren: Regex,
    siret: Regex,
    nir: Regex,
    card: Regex,
    ipv4: Regex,
    ipv6: Regex,
}

fn detectors() -> &'static Detectors {
    static DETECTORS: OnceLock<Detectors> = OnceLock::new();
    DETECTORS.get_or_init(|| Detectors {
        email: compile(
            r"[\p{L}\p{N}_%+-][\p{L}\p{N}._%+'-]*@[\p{L}\p{N}-]+(?:\.[\p{L}\p{N}-]+)*\.\p{L}{2,}",
        ),
        phone: compile(
            r"(?:\+33[ .-]?(?:\(0\)[ .-]?)?|0033[ .-]?|0)[1-9](?:[ .-]?\d{2}){4}|33[1-9]\d{8}",
        ),
        iban: compile(r"(?i)[a-z]{2}\d{2}(?: ?[a-z0-9]){11,32}"),
        siren: compile(r"\d{3}[ .]?\d{3}[ .]?\d{3}"),
        siret: compile(r"\d{3}[ .]?\d{3}[ .]?\d{3}[ .]?\d{5}"),
        nir: compile(
            r"[1-478][ .]?\d{2}[ .]?\d{2}[ .]?(?:\d{2}|2[ABab])[ .]?\d{3}[ .]?\d{3}[ .]?\d{2}",
        ),
        card: compile(r"\d(?:[ -]?\d){12,18}"),
        ipv4: compile(r"\d{1,3}(?:\.\d{1,3}){3}"),
        ipv6: compile(r"[0-9A-Fa-f]{0,4}(?::[0-9A-Fa-f]{0,4}){2,7}"),
    })
}

pub(crate) fn compile(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => panic!("invalid built-in pattern {pattern}: {error}"),
    }
}

pub(crate) fn scan(
    regex: &Regex,
    text: &str,
    out: &mut Vec<Span>,
    mut accept: impl FnMut(usize, usize) -> Option<(usize, Kind)>,
) {
    let mut position = 0;
    while position <= text.len() {
        let Some(found) = regex.find_at(text, position) else {
            break;
        };
        match accept(found.start(), found.end()) {
            Some((end, kind)) if end > found.start() => {
                out.push(Span {
                    start: found.start(),
                    end,
                    kind,
                });
                position = end;
            }
            _ => {
                position = found.start()
                    + text[found.start()..]
                        .chars()
                        .next()
                        .map_or(1, char::len_utf8);
            }
        }
    }
}

fn char_before(text: &str, index: usize) -> Option<char> {
    text[..index].chars().next_back()
}

fn char_after(text: &str, index: usize) -> Option<char> {
    text[index..].chars().next()
}

fn isolated(text: &str, start: usize, end: usize, extra: &[char]) -> bool {
    let clear = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && !extra.contains(&c));
    clear(char_before(text, start)) && clear(char_after(text, end))
}

fn compact(text: &str) -> String {
    text.chars().filter(char::is_ascii_alphanumeric).collect()
}

pub(crate) fn luhn(digits: &str) -> bool {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let sum: u32 = digits
        .bytes()
        .rev()
        .enumerate()
        .map(|(index, byte)| {
            let digit = u32::from(byte - b'0');
            if index % 2 == 1 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                digit
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

pub(crate) fn iban_valid(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if !(15..=34).contains(&bytes.len())
        || !bytes[..2].iter().all(u8::is_ascii_uppercase)
        || !bytes[2..4].iter().all(u8::is_ascii_digit)
        || !bytes
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return false;
    }
    if let Some((_, length)) = IBAN_LENGTHS
        .iter()
        .find(|(country, _)| country.as_bytes() == &bytes[..2])
        && *length != bytes.len()
    {
        return false;
    }
    let remainder = bytes[4..]
        .iter()
        .chain(&bytes[..4])
        .fold(0u32, |acc, &byte| {
            if byte.is_ascii_digit() {
                (acc * 10 + u32::from(byte - b'0')) % 97
            } else {
                (acc * 100 + u32::from(byte - b'A' + 10)) % 97
            }
        });
    remainder == 1
}

pub(crate) fn nir_valid(candidate: &str) -> bool {
    if candidate.len() != 15 || !candidate.is_ascii() {
        return false;
    }
    let (body, key) = candidate.split_at(13);
    let body = match &body[5..7] {
        "2A" | "2a" => format!("{}19{}", &body[..5], &body[7..]),
        "2B" | "2b" => format!("{}18{}", &body[..5], &body[7..]),
        _ => body.to_string(),
    };
    let (Ok(number), Ok(key)) = (body.parse::<u64>(), key.parse::<u64>()) else {
        return false;
    };
    body.bytes().all(|b| b.is_ascii_digit()) && 97 - number % 97 == key
}

pub(crate) fn french_phone_digits(digits: &str) -> bool {
    let bytes = digits.as_bytes();
    let national = |rest: &[u8]| rest.len() == 9 && rest[0] != b'0';
    match bytes.len() {
        9 => national(bytes),
        10 => bytes[0] == b'0' && national(&bytes[1..]),
        11 => bytes.starts_with(b"33") && national(&bytes[2..]),
        13 => bytes.starts_with(b"0033") && national(&bytes[4..]),
        _ => false,
    }
}

fn iban_end(text: &str, start: usize, end: usize) -> Option<usize> {
    let mut ends = Vec::new();
    let mut letters = String::new();
    for (offset, c) in text[start..end].char_indices() {
        if c.is_ascii_alphanumeric() {
            letters.push(c.to_ascii_uppercase());
            ends.push(start + offset + c.len_utf8());
        }
    }
    (15..=letters.len())
        .rev()
        .find(|&length| {
            iban_valid(&letters[..length]) && isolated(text, start, ends[length - 1], &[])
        })
        .map(|length| ends[length - 1])
}

pub(crate) fn pattern_spans(text: &str) -> Vec<Span> {
    let d = detectors();
    let mut spans = Vec::new();
    scan(&d.email, text, &mut spans, |_, end| {
        Some((end, Kind::Email))
    });
    scan(&d.phone, text, &mut spans, |start, end| {
        isolated(text, start, end, &[]).then_some((end, Kind::Phone))
    });
    scan(&d.iban, text, &mut spans, |start, end| {
        iban_end(text, start, end).map(|end| (end, Kind::Iban))
    });
    scan(&d.siren, text, &mut spans, |start, end| {
        (isolated(text, start, end, &[]) && luhn(&compact(&text[start..end])))
            .then_some((end, Kind::Siren))
    });
    scan(&d.siret, text, &mut spans, |start, end| {
        (isolated(text, start, end, &[]) && luhn(&compact(&text[start..end])))
            .then_some((end, Kind::Siret))
    });
    scan(&d.nir, text, &mut spans, |start, end| {
        (isolated(text, start, end, &[]) && nir_valid(&compact(&text[start..end])))
            .then_some((end, Kind::Nir))
    });
    scan(&d.card, text, &mut spans, |start, end| {
        (isolated(text, start, end, &[]) && luhn(&compact(&text[start..end])))
            .then_some((end, Kind::CardNumber))
    });
    scan(&d.ipv4, text, &mut spans, |start, end| {
        let valid = text[start..end]
            .split('.')
            .all(|octet| octet.parse::<u8>().is_ok());
        (valid && isolated(text, start, end, &['.'])).then_some((end, Kind::IpAddress))
    });
    scan(&d.ipv6, text, &mut spans, |start, end| {
        let candidate = &text[start..end];
        let valid = candidate.bytes().any(|b| b.is_ascii_hexdigit())
            && candidate.parse::<Ipv6Addr>().is_ok();
        (valid && isolated(text, start, end, &[':'])).then_some((end, Kind::IpAddress))
    });
    spans
}

fn priority(kind: Kind) -> u8 {
    match kind {
        Kind::Email => 0,
        Kind::Iban => 1,
        Kind::Nir => 2,
        Kind::Siret => 3,
        Kind::Siren => 4,
        Kind::CardNumber => 5,
        Kind::Phone => 6,
        Kind::IpAddress => 7,
        Kind::Address => 8,
        Kind::PersonName => 9,
        Kind::LastName => 10,
        Kind::FirstName => 11,
        _ => 12,
    }
}

fn resolve(mut candidates: Vec<Span>) -> Vec<Span> {
    candidates.sort_by_key(|span| {
        (
            std::cmp::Reverse(span.end - span.start),
            priority(span.kind),
            span.start,
        )
    });
    let mut kept: Vec<Span> = Vec::new();
    for span in candidates {
        if kept
            .iter()
            .all(|other| span.end <= other.start || other.end <= span.start)
        {
            kept.push(span);
        }
    }
    kept.sort_by_key(|span| span.start);
    kept
}

fn is_name(kind: Kind) -> bool {
    matches!(kind, Kind::PersonName | Kind::LastName | Kind::FirstName)
}

fn merge_names(text: &str, spans: Vec<Span>) -> Vec<Span> {
    let mut merged: Vec<Span> = Vec::with_capacity(spans.len());
    for span in spans {
        if let Some(last) = merged.last_mut() {
            let gap = &text[last.end..span.start];
            if is_name(last.kind)
                && is_name(span.kind)
                && !gap.is_empty()
                && gap.chars().all(|c| c == ' ')
            {
                last.end = span.end;
                last.kind = Kind::PersonName;
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

pub fn detect_spans(text: &str) -> Vec<Span> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let mut candidates = pattern_spans(text);
    candidates.extend(freetext::address_spans(text));
    candidates.extend(freetext::name_spans(text));
    merge_names(text, resolve(candidates))
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn luhn_accepte_un_siren_connu_et_refuse_une_faute_de_frappe() {
        assert!(luhn("732829320"));
        assert!(!luhn("732829321"));
    }

    #[test]
    fn un_iban_verifie_le_pays_la_longueur_et_le_modulo_97() {
        assert!(iban_valid("FR7630006000011234567890189"));
        assert!(!iban_valid("FR7630006000011234567890188"));
        assert!(!iban_valid("FR763000600001123456789018"));
        assert!(iban_valid("FR1420041010050500013M02606"));
    }

    #[test]
    fn la_cle_du_nir_est_97_moins_le_corps_modulo_97() {
        assert!(nir_valid("285117512345622"));
        assert!(!nir_valid("285117512345623"));
        let corsica_body: u64 = 1_850_219_012_345;
        let key = 97 - corsica_body % 97;
        assert!(nir_valid(&format!("185022A012345{key:02}")));
    }

    #[test]
    fn un_telephone_francais_accepte_le_zero_perdu_et_l_indicatif_pays() {
        assert!(french_phone_digits("0612345678"));
        assert!(french_phone_digits("612345678"));
        assert!(french_phone_digits("33612345678"));
        assert!(!french_phone_digits("0012345678"));
        assert!(!french_phone_digits("12345"));
    }

    #[test]
    fn les_variantes_de_telephone_sont_detectees() {
        for text in [
            "06 12 34 56 78",
            "06.12.34.56.78",
            "06-12-34-56-78",
            "0612345678",
            "+33 6 12 34 56 78",
            "+33612345678",
            "0033 6 12 34 56 78",
        ] {
            let spans = detect_spans(text);
            assert_eq!(spans.len(), 1, "{text}");
            assert_eq!(spans[0].kind, Kind::Phone, "{text}");
            assert_eq!(&text[spans[0].start..spans[0].end], text, "{text}");
        }
    }

    #[test]
    fn des_segments_qui_se_chevauchent_gardent_le_plus_long() {
        let text = "SIRET 732 829 320 00074";
        let spans = detect_spans(text);
        assert_eq!(spans.len(), 1);
        assert_eq!(&text[spans[0].start..spans[0].end], "732 829 320 00074");
    }
}
