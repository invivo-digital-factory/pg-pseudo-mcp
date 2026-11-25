use std::net::Ipv6Addr;

use cucumber::{given, then};
use fake::locales::{Data, FR_FR};

use crate::anonymization_world::{AnonymizationWorld, digits, layout, listed};

const FICTIONAL_PHONE_BLOCKS: [&str; 7] = [
    "019900", "026191", "035373", "046571", "053649", "063998", "073998",
];

fn luhn_valid(number: &str) -> bool {
    let sum: u32 = number
        .chars()
        .rev()
        .enumerate()
        .map(|(position, c)| {
            let digit = c.to_digit(10).expect("a digit");
            if position % 2 == 1 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                digit
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn mod97(numeric: &str) -> u64 {
    numeric.chars().fold(0, |acc, c| {
        (acc * 10 + u64::from(c.to_digit(10).expect("a digit"))) % 97
    })
}

fn iban_valid(iban: &str) -> bool {
    let compact: String = iban.chars().filter(|c| !c.is_whitespace()).collect();
    let rearranged = format!("{}{}", &compact[4..], &compact[..4]);
    let numeric: String = rearranged
        .chars()
        .map(|c| c.to_digit(36).expect("alphanumeric").to_string())
        .collect();
    mod97(&numeric) == 1
}

pub fn in_fictional_phone_block(text: &str) -> bool {
    let all = digits(text);
    if all.len() < 9 {
        return false;
    }
    let national = format!("0{}", &all[all.len() - 9..]);
    FICTIONAL_PHONE_BLOCKS
        .iter()
        .any(|block| national.starts_with(block))
}

pub fn is_catalogue_first_name(name: &str) -> bool {
    <FR_FR as Data>::NAME_FIRST_NAME
        .iter()
        .any(|candidate| candidate.to_lowercase() == name.to_lowercase())
}

fn is_name_word(word: &str) -> bool {
    word.chars()
        .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
}

#[given(regex = r"^l'espace des téléphones fictifs est limité à (\d+) numéros?$")]
async fn phone_pool_is_limited(world: &mut AnonymizationWorld, limit: usize) {
    world.generator.phone_pool_limit = Some(limit);
}

#[then(regex = r"^le pseudonyme est un numéro des blocs fictifs, présenté comme l'original$")]
async fn pseudonym_is_fictional_phone_like_input(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(layout(pseudonym), layout(world.input()), "{pseudonym}");
    assert!(in_fictional_phone_block(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est un IBAN valide de la banque fictive 99999$")]
async fn pseudonym_is_fictional_iban(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let compact: String = pseudonym.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(iban_valid(pseudonym), "{pseudonym}");
    assert!(compact.starts_with("FR"), "{pseudonym}");
    assert_eq!(&compact[4..9], "99999", "{pseudonym}");
    assert_eq!(layout(pseudonym), layout(world.input()), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme a 9 chiffres et une clé de Luhn valide$")]
async fn pseudonym_is_siren(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(pseudonym.len(), 9, "{pseudonym}");
    assert!(luhn_valid(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme a 14 chiffres, une clé de Luhn valide et un SIREN valide$")]
async fn pseudonym_is_siret(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(pseudonym.len(), 14, "{pseudonym}");
    assert!(luhn_valid(pseudonym), "{pseudonym}");
    assert!(luhn_valid(&pseudonym[..9]), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est un NIR à clé valide qui garde le sexe de l'original$")]
async fn pseudonym_is_nir(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let nir = digits(pseudonym);
    let body: u64 = nir[..13].parse().expect("a NIR body");
    let key: u64 = nir[13..].parse().expect("a NIR key");
    assert_eq!(key, 97 - body % 97, "{pseudonym}");
    assert_eq!(nir[..1], digits(world.input())[..1], "{pseudonym}");
    let month: u32 = nir[3..5].parse().expect("a month");
    assert!((1..=12).contains(&month), "{pseudonym}");
    assert_eq!(layout(pseudonym), layout(world.input()), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme commence par 400000 et passe le contrôle de Luhn$")]
async fn pseudonym_is_card(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let card = digits(pseudonym);
    assert!(card.starts_with("400000"), "{pseudonym}");
    assert!(luhn_valid(&card), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est une adresse du préfixe IPv6 de documentation 2001:db8$")]
async fn pseudonym_is_documentation_ipv6(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let address: Ipv6Addr = pseudonym.parse().expect("an IPv6 address");
    assert_eq!(address.segments()[..2], [0x2001, 0x0db8], "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est un prénom du catalogue suivi d'un nom de famille$")]
async fn pseudonym_is_person_name(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let (first, last) = pseudonym.rsplit_once(' ').expect("two words");
    assert!(is_catalogue_first_name(first), "{pseudonym}");
    assert!(is_name_word(last), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est un seul nom de famille$")]
async fn pseudonym_is_last_name(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert!(!pseudonym.contains(' '), "{pseudonym}");
    assert!(is_name_word(pseudonym), "{pseudonym}");
    assert!(pseudonym.chars().count() >= 3, "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est un prénom du catalogue$")]
async fn pseudonym_is_first_name(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert!(is_catalogue_first_name(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme commence par un numéro suivi du même type de voie$")]
async fn pseudonym_is_address(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let words: Vec<&str> = pseudonym.split_whitespace().collect();
    let original: Vec<&str> = world.input().split_whitespace().collect();
    assert!(words[0].parse::<u32>().is_ok(), "{pseudonym}");
    assert_eq!(words[1], original[1], "{pseudonym}");
    assert!(words.len() > 2, "{pseudonym}");
}

#[then(regex = r"^le pseudonyme a (\d+) chiffres$")]
async fn pseudonym_has_digits(world: &mut AnonymizationWorld, count: usize) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(pseudonym.len(), count, "{pseudonym}");
    assert!(pseudonym.chars().all(|c| c.is_ascii_digit()), "{pseudonym}");
}

#[then(regex = r#"^le pseudonyme est l'une des valeurs "([^"]+)"$"#)]
async fn pseudonym_is_one_of(world: &mut AnonymizationWorld, values: String) {
    let pseudonym = world.last_pseudonym();
    assert!(
        listed(&values).iter().any(|value| value == pseudonym),
        "{pseudonym} not in {values}"
    );
}

#[then(regex = r"^le pseudonyme (\d+) est un numéro des blocs fictifs$")]
async fn nth_pseudonym_is_fictional_phone(world: &mut AnonymizationWorld, position: usize) {
    let pseudonym = world.nth_pseudonym(position);
    assert!(in_fictional_phone_block(pseudonym), "{pseudonym}");
}

#[then(regex = r#"^le pseudonyme (\d+) est un jeton opaque "([^"]+)"$"#)]
async fn nth_pseudonym_is_token(world: &mut AnonymizationWorld, position: usize, label: String) {
    let pseudonym = world.nth_pseudonym(position);
    let id = pseudonym
        .strip_prefix(&format!("<{label} id="))
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or_else(|| panic!("{pseudonym} is not a {label} token"));
    assert_eq!(id.len(), 8, "{pseudonym}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme (\d+) est identique au pseudonyme (\d+)$")]
async fn nth_pseudonyms_are_identical(world: &mut AnonymizationWorld, left: usize, right: usize) {
    assert_eq!(world.nth_pseudonym(left), world.nth_pseudonym(right));
}

#[then(regex = r"^le pseudonyme (\d+) ne se traduit pas en retour$")]
async fn nth_pseudonym_does_not_translate(world: &mut AnonymizationWorld, position: usize) {
    let pseudonym = world.nth_pseudonym(position).to_string();
    let translation = world.use_cases().translate_pseudonym.execute(&pseudonym);
    assert_eq!(translation.real, None, "{pseudonym}");
}
