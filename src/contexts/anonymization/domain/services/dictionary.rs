use std::sync::OnceLock;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::contexts::anonymization::domain::types::Kind;

const IDENTIFIERS: &str = include_str!("../../../../../data/identifier.csv");

const SUPPLEMENT: &[(&str, &str)] = &[
    ("nir", "social_security_id"),
    ("mail", "email"),
    ("tel", "phone_number"),
    ("surname", "name"),
    ("rib", "iban"),
];

struct Entry {
    tokens: Vec<String>,
    kind: Option<Kind>,
}

struct Hit {
    start: usize,
    len: usize,
    kind: Option<Kind>,
}

pub(crate) fn fold(text: &str) -> String {
    text.nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn tokenize(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();
    for (index, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            flush(&mut current, &mut tokens);
            continue;
        }
        if !current.is_empty() && index > 0 {
            let prev = chars[index - 1];
            let next = chars.get(index + 1).copied();
            let camel = prev.is_lowercase() && c.is_uppercase();
            let digit_edge = prev.is_alphabetic() != c.is_alphabetic();
            let acronym_end =
                prev.is_uppercase() && c.is_uppercase() && next.is_some_and(char::is_lowercase);
            if camel || digit_edge || acronym_end {
                flush(&mut current, &mut tokens);
            }
        }
        current.push(c);
    }
    flush(&mut current, &mut tokens);
    tokens
}

fn flush(current: &mut String, tokens: &mut Vec<String>) {
    if !current.is_empty() {
        tokens.push(fold(current));
        current.clear();
    }
}

fn category_kind(category: &str) -> Option<Kind> {
    match category {
        "name" | "lastname" => Some(Kind::LastName),
        "firstname" => Some(Kind::FirstName),
        "email" => Some(Kind::Email),
        "phone_number" | "fax_number" => Some(Kind::Phone),
        "iban" => Some(Kind::Iban),
        "siret_france" => Some(Kind::Siret),
        "siren_france" => Some(Kind::Siren),
        "social_security_id" => Some(Kind::Nir),
        "birth_date" => Some(Kind::BirthDate),
        "street_address" => Some(Kind::Address),
        "city" => Some(Kind::City),
        "zipcode_france" | "zipcode" => Some(Kind::PostalCode),
        "civility" => Some(Kind::Civility),
        "gender" => Some(Kind::Gender),
        "creditcard" => Some(Kind::CardNumber),
        "ip_address" => Some(Kind::IpAddress),
        _ => None,
    }
}

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let listed = IDENTIFIERS.lines().filter_map(|line| {
            let mut fields = line.split('\t');
            let _lang = fields.next()?;
            let word = fields.next()?;
            let category = fields.next()?;
            Some((word.trim(), category.trim()))
        });
        listed
            .chain(SUPPLEMENT.iter().copied())
            .map(|(word, category)| Entry {
                tokens: tokenize(word),
                kind: category_kind(category),
            })
            .filter(|entry| !entry.tokens.is_empty())
            .collect()
    })
}

fn hits(tokens: &[String]) -> Vec<Hit> {
    let mut found = Vec::new();
    for entry in entries() {
        let len = entry.tokens.len();
        if len > tokens.len() {
            continue;
        }
        for start in 0..=tokens.len() - len {
            if tokens[start..start + len] == entry.tokens[..] {
                found.push(Hit {
                    start,
                    len,
                    kind: entry.kind,
                });
            }
        }
    }
    found
}

fn covers(outer: &Hit, inner: &Hit) -> bool {
    outer.len > inner.len
        && outer.start <= inner.start
        && inner.start + inner.len <= outer.start + outer.len
}

pub fn dictionary_kind(column_name: &str) -> Option<Kind> {
    dictionary_entry(column_name).flatten()
}

pub fn dictionary_entry(column_name: &str) -> Option<Option<Kind>> {
    let tokens = tokenize(column_name);
    let found = hits(&tokens);
    if found.is_empty() {
        return None;
    }
    let kind = found
        .iter()
        .filter(|hit| hit.kind.is_some())
        .filter(|hit| {
            !found
                .iter()
                .any(|other| other.kind.is_none() && covers(other, hit))
        })
        .min_by_key(|hit| (std::cmp::Reverse(hit.len), hit.start))
        .and_then(|hit| hit.kind);
    Some(kind)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn le_decoupage_separe_snake_kebab_camel_et_chiffres() {
        assert_eq!(tokenize("telephonePortable"), vec!["telephone", "portable"]);
        assert_eq!(tokenize("IPAddress"), vec!["ip", "address"]);
        assert_eq!(
            tokenize("date-de_naissance2"),
            vec!["date", "de", "naissance", "2"]
        );
        assert_eq!(tokenize("Prénom"), vec!["prenom"]);
    }

    #[test]
    fn la_correspondance_personnelle_la_plus_longue_l_emporte() {
        assert_eq!(dictionary_kind("email_address"), Some(Kind::Email));
        assert_eq!(dictionary_kind("nom_famille"), Some(Kind::LastName));
        assert_eq!(dictionary_kind("adresse_ip"), Some(Kind::IpAddress));
    }

    #[test]
    fn une_categorie_ignoree_protege_les_mots_qu_elle_contient() {
        assert_eq!(dictionary_kind("nom_utilisateur"), None);
        assert_eq!(dictionary_kind("mac_address"), None);
        assert_eq!(dictionary_kind("client_id"), None);
    }

    #[test]
    fn une_entree_ignoree_se_distingue_d_un_nom_absent_du_dictionnaire() {
        assert_eq!(dictionary_entry("client_id"), Some(None));
        assert_eq!(dictionary_entry("telephone"), Some(Some(Kind::Phone)));
        assert_eq!(dictionary_entry("valeur"), None);
    }

    #[test]
    fn chaque_ligne_du_dictionnaire_a_trois_champs() {
        assert!(
            IDENTIFIERS
                .lines()
                .all(|line| line.split('\t').count() == 3)
        );
        assert!(entries().len() > 150);
    }
}
