use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::LazyLock;

use fake::locales::{Data, FR_FR};
use rand::Rng;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind};

pub(crate) const RESERVED_EMAIL_DOMAINS: [&str; 3] = ["example.com", "example.net", "example.org"];

pub(crate) const FICTIONAL_PHONE_BLOCKS: [&str; 7] = [
    "019900", "026191", "035373", "046571", "053649", "063998", "073998",
];
const PHONE_BLOCK_SIZE: usize = 10_000;

const COPIED_SEPARATORS: &str = " .-+()/";
const MAX_PREFIX_LENGTH_DIGITS: usize = 3;

static TIME_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[ T][0-9]{2}:[0-9]{2}(:[0-9]{2}(\.[0-9]{1,6})?)?(Z|[+-][0-9]{2}(:?[0-9]{2})?)?$")
        .expect("time suffix pattern is valid")
});

const PSEUDONYM_BANK_CODE: &str = "99999";
const CARD_PREFIX: &str = "400000";
const DOCUMENTATION_IPV4_NETWORKS: [[u8; 3]; 3] = [[192, 0, 2], [198, 51, 100], [203, 0, 113]];
const DOCUMENTATION_IPV6_PREFIX: [u16; 2] = [0x2001, 0x0db8];

const NAME_ONSETS: [&str; 24] = [
    "b", "bl", "br", "c", "ch", "cl", "d", "f", "fr", "g", "gr", "gu", "j", "l", "m", "n", "p",
    "pl", "pr", "r", "s", "t", "tr", "v",
];
const NAME_VOWELS: [&str; 10] = ["a", "e", "i", "o", "ou", "au", "an", "on", "é", "ai"];
const NAME_ENDINGS: [&str; 30] = [
    "rcy", "t", "n", "l", "ret", "chon", "lier", "rand", "mont", "ville", "court", "geot", "det",
    "nel", "rin", "lard", "sson", "quet", "vier", "teau", "ssier", "doux", "rel", "bert", "lin",
    "not", "zet", "gnac", "rieux", "llet",
];
const NAME_SYLLABLE_COUNTS: [usize; 3] = [2, 2, 3];

const CITIES: [&str; 60] = [
    "Quimperlé",
    "Vannes",
    "Saint-Malo",
    "Dinan",
    "Laval",
    "Angers",
    "Cholet",
    "Niort",
    "Poitiers",
    "Limoges",
    "Brive-la-Gaillarde",
    "Aurillac",
    "Rodez",
    "Albi",
    "Castres",
    "Agen",
    "Pau",
    "Bayonne",
    "Dax",
    "Mont-de-Marsan",
    "Périgueux",
    "Angoulême",
    "Cognac",
    "Saintes",
    "Rochefort",
    "Royan",
    "Bourges",
    "Vierzon",
    "Nevers",
    "Auxerre",
    "Sens",
    "Troyes",
    "Chaumont",
    "Épinal",
    "Colmar",
    "Mulhouse",
    "Belfort",
    "Vesoul",
    "Dole",
    "Bourg-en-Bresse",
    "Annecy",
    "Chambéry",
    "Valence",
    "Montélimar",
    "Nîmes",
    "Alès",
    "Béziers",
    "Sète",
    "Carcassonne",
    "Perpignan",
    "Tarbes",
    "Auch",
    "Montauban",
    "Cahors",
    "Figeac",
    "Moulins",
    "Vichy",
    "Le Puy-en-Velay",
    "Mende",
    "Gap",
];

const STREET_TYPES: [&str; 40] = [
    "rue",
    "avenue",
    "av.",
    "av",
    "boulevard",
    "bd",
    "bld",
    "chemin",
    "ch.",
    "impasse",
    "imp.",
    "allée",
    "allee",
    "place",
    "pl.",
    "route",
    "rte",
    "quai",
    "cours",
    "square",
    "voie",
    "passage",
    "résidence",
    "residence",
    "chaussée",
    "sentier",
    "faubourg",
    "fbg",
    "lotissement",
    "lieu-dit",
    "hameau",
    "montée",
    "rond-point",
    "promenade",
    "esplanade",
    "cité",
    "sente",
    "traverse",
    "clos",
    "parvis",
];

const STREET_NAMES: [&str; 25] = [
    "des Lilas",
    "du Moulin",
    "de la Gare",
    "des Écoles",
    "du Château",
    "de l'Église",
    "des Tilleuls",
    "du Stade",
    "de la Fontaine",
    "des Acacias",
    "du Port",
    "des Vignes",
    "de la Mairie",
    "du Pré",
    "des Peupliers",
    "du Lavoir",
    "de la Forge",
    "des Chênes",
    "du Marché",
    "de la Croix",
    "des Jardins",
    "du Calvaire",
    "de la Source",
    "des Roses",
    "du Parc",
];

const CIVILITY_SETS: [[&str; 2]; 8] = [
    ["M.", "Mme"],
    ["M", "Mme"],
    ["M.", "Mlle"],
    ["Monsieur", "Madame"],
    ["Monsieur", "Mademoiselle"],
    ["Mr", "Mrs"],
    ["Mr.", "Mrs."],
    ["Mr", "Ms"],
];

const GENDER_SETS: [[&str; 2]; 6] = [
    ["M", "F"],
    ["H", "F"],
    ["Homme", "Femme"],
    ["Masculin", "Féminin"],
    ["Male", "Female"],
    ["1", "2"],
];

pub(crate) fn generate<R: Rng + ?Sized>(
    kind: Kind,
    value: &str,
    rng: &mut R,
    config: &GeneratorConfig,
) -> String {
    match kind {
        Kind::Email => email(rng),
        Kind::Phone => phone(value, rng, config),
        Kind::Iban => iban(value, rng),
        Kind::Siren => overlay_digits(value, &siren(rng)),
        Kind::Siret => overlay_digits(value, &siret(rng)),
        Kind::Nir => nir(value, rng),
        Kind::CardNumber => overlay_digits(value, &card_number(rng)),
        Kind::IpAddress => ip_address(value, rng),
        Kind::PersonName => person_name(value, rng),
        Kind::LastName => apply_case(&last_name(rng), case_of(value)),
        Kind::FirstName => apply_case(first_name(rng), case_of(value)),
        Kind::Address => address(value, rng),
        Kind::City => apply_case(pick(rng, &CITIES), case_of(value)),
        Kind::PostalCode => overlay_digits(value, &postal_code(rng)),
        Kind::Civility => pick_in_matching_set(value, &CIVILITY_SETS, rng),
        Kind::Gender => pick_in_matching_set(value, &GENDER_SETS, rng),
        Kind::BirthDate => birth_date(value, rng),
    }
}

pub(crate) fn recognise(text: &str) -> Option<Kind> {
    let text = text.trim();
    if is_reserved_email(text) {
        Some(Kind::Email)
    } else if is_fictional_phone(text) {
        Some(Kind::Phone)
    } else if is_pseudonym_iban(text) {
        Some(Kind::Iban)
    } else if is_documentation_ip(text) {
        Some(Kind::IpAddress)
    } else {
        None
    }
}

fn below<R: Rng + ?Sized>(rng: &mut R, bound: usize) -> usize {
    (rng.next_u64() % bound as u64) as usize
}

fn pick<'a, R: Rng + ?Sized>(rng: &mut R, items: &[&'a str]) -> &'a str {
    items[below(rng, items.len())]
}

fn random_digits<R: Rng + ?Sized>(rng: &mut R, count: usize) -> String {
    (0..count)
        .map(|_| char::from(b'0' + below(rng, 10) as u8))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Case {
    Upper,
    Lower,
    AsIs,
}

fn case_of(text: &str) -> Case {
    let letters: Vec<char> = text.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        Case::Upper
    } else if !letters.is_empty() && letters.iter().all(|c| c.is_lowercase()) {
        Case::Lower
    } else {
        Case::AsIs
    }
}

fn apply_case(text: &str, case: Case) -> String {
    match case {
        Case::Upper => text.to_uppercase(),
        Case::Lower => text.to_lowercase(),
        Case::AsIs => text.to_string(),
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn overlay(template: &str, replacement: &str, is_slot: impl Fn(char) -> bool) -> Option<String> {
    if template
        .chars()
        .any(|c| !is_slot(c) && !COPIED_SEPARATORS.contains(c))
    {
        return None;
    }
    if template.chars().filter(|c| is_slot(*c)).count() != replacement.chars().count() {
        return None;
    }
    let mut fill = replacement.chars();
    Some(
        template
            .chars()
            .map(|c| {
                if is_slot(c) {
                    fill.next().unwrap_or(c)
                } else {
                    c
                }
            })
            .collect(),
    )
}

fn overlay_digits(template: &str, digits: &str) -> String {
    overlay(template, digits, |c| c.is_ascii_digit()).unwrap_or_else(|| digits.to_string())
}

fn luhn_check_digit(payload: &str) -> char {
    let sum: u32 = payload
        .bytes()
        .rev()
        .enumerate()
        .map(|(position, byte)| {
            let digit = u32::from(byte - b'0');
            if position % 2 == 0 {
                let doubled = digit * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                digit
            }
        })
        .sum();
    char::from(b'0' + ((10 - sum % 10) % 10) as u8)
}

fn with_luhn(payload: String) -> String {
    let check = luhn_check_digit(&payload);
    let mut number = payload;
    number.push(check);
    number
}

fn mod97(numeric: &str) -> u64 {
    numeric
        .chars()
        .filter_map(|c| c.to_digit(10))
        .fold(0, |acc, digit| (acc * 10 + u64::from(digit)) % 97)
}

fn letters_to_digits(text: &str) -> String {
    text.chars()
        .filter_map(|c| c.to_digit(36))
        .map(|value| value.to_string())
        .collect()
}

fn ascii_slug(text: &str) -> String {
    let mut slug = String::new();
    for c in text.nfd().filter(|c| !is_combining_mark(*c)) {
        match c {
            'œ' | 'Œ' => slug.push_str("oe"),
            'æ' | 'Æ' => slug.push_str("ae"),
            c if c.is_ascii_alphanumeric() => slug.push(c.to_ascii_lowercase()),
            _ if !slug.is_empty() && !slug.ends_with('-') => slug.push('-'),
            _ => {}
        }
    }
    slug.trim_end_matches('-').to_string()
}

fn first_name<R: Rng + ?Sized>(rng: &mut R) -> &'static str {
    pick(rng, <FR_FR as Data>::NAME_FIRST_NAME)
}

fn last_name<R: Rng + ?Sized>(rng: &mut R) -> String {
    let syllables = NAME_SYLLABLE_COUNTS[below(rng, NAME_SYLLABLE_COUNTS.len())];
    let mut name = String::new();
    for _ in 0..syllables {
        name.push_str(pick(rng, &NAME_ONSETS));
        name.push_str(pick(rng, &NAME_VOWELS));
    }
    name.push_str(pick(rng, &NAME_ENDINGS));
    capitalize(&name)
}

fn email<R: Rng + ?Sized>(rng: &mut R) -> String {
    let first = ascii_slug(first_name(rng));
    let last = ascii_slug(&last_name(rng));
    let number = below(rng, 100);
    let domain = pick(rng, &RESERVED_EMAIL_DOMAINS);
    format!("{first}.{last}{number}@{domain}")
}

fn is_mobile_block(block: &str) -> bool {
    block.starts_with("06") || block.starts_with("07")
}

fn phone<R: Rng + ?Sized>(value: &str, rng: &mut R, config: &GeneratorConfig) -> String {
    let digits: Vec<char> = value.chars().filter(char::is_ascii_digit).collect();
    let mobile = digits.len() >= 9 && matches!(digits[digits.len() - 9], '6' | '7');
    let blocks: Vec<&str> = FICTIONAL_PHONE_BLOCKS
        .iter()
        .copied()
        .filter(|block| is_mobile_block(block) == mobile)
        .collect();
    let capacity = blocks.len() * PHONE_BLOCK_SIZE;
    let pool = config
        .phone_pool_limit
        .map_or(capacity, |limit| limit.clamp(1, capacity));
    let index = below(rng, pool);
    let national = format!(
        "{}{:04}",
        blocks[index / PHONE_BLOCK_SIZE],
        index % PHONE_BLOCK_SIZE
    );
    if !is_french_phone_shape(value, &digits) {
        return group_in_pairs(&national);
    }
    let kept_prefix = digits.len() - 9;
    let mut subscriber = national[1..].chars();
    let mut seen = 0;
    value
        .chars()
        .map(|c| {
            if !c.is_ascii_digit() {
                return c;
            }
            seen += 1;
            if seen > kept_prefix {
                subscriber.next().unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

fn is_french_phone_shape(value: &str, digits: &[char]) -> bool {
    if value
        .chars()
        .any(|c| !c.is_ascii_digit() && !COPIED_SEPARATORS.contains(c))
    {
        return false;
    }
    let digits: String = digits.iter().collect();
    match digits.len() {
        10 => digits.starts_with('0'),
        11 => digits.starts_with("33"),
        12 => digits.starts_with("330"),
        13 => digits.starts_with("0033"),
        _ => false,
    }
}

fn group_in_pairs(national: &str) -> String {
    let mut grouped = String::new();
    for (position, c) in national.chars().enumerate() {
        if position > 0 && position % 2 == 0 {
            grouped.push(' ');
        }
        grouped.push(c);
    }
    grouped
}

fn is_fictional_phone(text: &str) -> bool {
    if text.is_empty()
        || !text
            .chars()
            .all(|c| c.is_ascii_digit() || " .-+()".contains(c))
    {
        return false;
    }
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    let subscriber = match digits.len() {
        10 if digits.starts_with('0') => &digits[1..],
        11 if digits.starts_with("33") => &digits[2..],
        12 if digits.starts_with("330") => &digits[3..],
        13 if digits.starts_with("0033") => &digits[4..],
        _ => return false,
    };
    FICTIONAL_PHONE_BLOCKS
        .iter()
        .any(|block| subscriber.starts_with(&block[1..]))
}

fn rib_key(bank: &str, branch: &str, account: &str) -> u64 {
    let weighted = format!(
        "{}",
        89 * mod97(bank) + 15 * mod97(branch) + 3 * mod97(account)
    );
    97 - mod97(&weighted)
}

fn iban_check_digits(country: &str, bban: &str) -> u64 {
    let numeric = format!("{bban}{}00", letters_to_digits(country));
    98 - mod97(&numeric)
}

fn iban<R: Rng + ?Sized>(value: &str, rng: &mut R) -> String {
    let branch = random_digits(rng, 5);
    let account = random_digits(rng, 11);
    let key = rib_key(PSEUDONYM_BANK_CODE, &branch, &account);
    let bban = format!("{PSEUDONYM_BANK_CODE}{branch}{account}{key:02}");
    let compact = format!("FR{:02}{bban}", iban_check_digits("FR", &bban));
    if let Some(laid_out) = overlay(value, &compact, |c| c.is_ascii_alphanumeric()) {
        return laid_out;
    }
    if value.contains(' ') {
        compact
            .as_bytes()
            .chunks(4)
            .map(|chunk| String::from_utf8_lossy(chunk).into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        compact
    }
}

fn is_pseudonym_iban(text: &str) -> bool {
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    compact.len() == 27
        && compact.chars().all(|c| c.is_ascii_alphanumeric())
        && compact.starts_with("FR")
        && &compact[4..9] == PSEUDONYM_BANK_CODE
        && mod97(&format!(
            "{}{}",
            letters_to_digits(&compact[4..]),
            letters_to_digits(&compact[..4])
        )) == 1
}

fn siren<R: Rng + ?Sized>(rng: &mut R) -> String {
    let lead = char::from(b'1' + below(rng, 9) as u8);
    with_luhn(format!("{lead}{}", random_digits(rng, 7)))
}

fn siret<R: Rng + ?Sized>(rng: &mut R) -> String {
    let siren = siren(rng);
    with_luhn(format!("{siren}{}", random_digits(rng, 4)))
}

fn card_number<R: Rng + ?Sized>(rng: &mut R) -> String {
    with_luhn(format!("{CARD_PREFIX}{}", random_digits(rng, 9)))
}

fn nir<R: Rng + ?Sized>(value: &str, rng: &mut R) -> String {
    let sex = match value.trim_start().chars().next() {
        Some(c @ ('1' | '2')) => c,
        _ => char::from(b'1' + below(rng, 2) as u8),
    };
    let year = below(rng, 100);
    let month = 1 + below(rng, 12);
    let department = match 1 + below(rng, 95) {
        20 => 19,
        other => other,
    };
    let commune = 1 + below(rng, 990);
    let order = 1 + below(rng, 999);
    let body = format!("{sex}{year:02}{month:02}{department:02}{commune:03}{order:03}");
    let key = 97 - mod97(&body);
    let digit_count = value.chars().filter(char::is_ascii_digit).count();
    if digit_count == body.len() {
        return overlay_digits(value, &body);
    }
    overlay_digits(value, &format!("{body}{key:02}"))
}

fn ip_address<R: Rng + ?Sized>(value: &str, rng: &mut R) -> String {
    let (address, suffix) = value.split_at(value.find('/').unwrap_or(value.len()));
    let suffix = if is_prefix_length(suffix) { suffix } else { "" };
    let generated = if address.contains(':') {
        let mut segment = || below(rng, 0x1_0000) as u16;
        Ipv6Addr::new(
            DOCUMENTATION_IPV6_PREFIX[0],
            DOCUMENTATION_IPV6_PREFIX[1],
            segment(),
            segment(),
            segment(),
            segment(),
            segment(),
            segment(),
        )
        .to_string()
    } else {
        let [a, b, c] = DOCUMENTATION_IPV4_NETWORKS[below(rng, DOCUMENTATION_IPV4_NETWORKS.len())];
        Ipv4Addr::new(a, b, c, 1 + below(rng, 254) as u8).to_string()
    };
    format!("{generated}{suffix}")
}

fn is_prefix_length(suffix: &str) -> bool {
    suffix.strip_prefix('/').is_some_and(|length| {
        (1..=MAX_PREFIX_LENGTH_DIGITS).contains(&length.len())
            && length.bytes().all(|b| b.is_ascii_digit())
    })
}

fn is_documentation_ip(text: &str) -> bool {
    let address = text.split('/').next().unwrap_or(text);
    match address.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => DOCUMENTATION_IPV4_NETWORKS
            .iter()
            .any(|network| v4.octets()[..3] == network[..]),
        Ok(IpAddr::V6(v6)) => v6.segments()[..2] == DOCUMENTATION_IPV6_PREFIX,
        Err(_) => false,
    }
}

fn is_reserved_email(text: &str) -> bool {
    text.rsplit_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && !local.contains(char::is_whitespace)
            && RESERVED_EMAIL_DOMAINS
                .iter()
                .any(|reserved| domain.eq_ignore_ascii_case(reserved))
    })
}

fn person_name<R: Rng + ?Sized>(value: &str, rng: &mut R) -> String {
    let words: Vec<&str> = value.split_whitespace().collect();
    match words.as_slice() {
        [first, .., last] => {
            let first_name = apply_case(first_name(rng), case_of(first));
            let last_name = apply_case(&last_name(rng), case_of(last));
            format!("{first_name} {last_name}")
        }
        _ => apply_case(&last_name(rng), case_of(value)),
    }
}

fn street_name<R: Rng + ?Sized>(rng: &mut R) -> String {
    if below(rng, 3) == 0 {
        let first = first_name(rng);
        format!("{first} {}", last_name(rng))
    } else {
        pick(rng, &STREET_NAMES).to_string()
    }
}

fn postal_code<R: Rng + ?Sized>(rng: &mut R) -> String {
    let department = match 1 + below(rng, 95) {
        20 => 19,
        other => other,
    };
    format!("{department:02}{:03}", below(rng, 1000))
}

fn address<R: Rng + ?Sized>(value: &str, rng: &mut R) -> String {
    let words: Vec<&str> = value.split_whitespace().collect();
    let has_number = words
        .first()
        .is_some_and(|word| word.starts_with(|c: char| c.is_ascii_digit()));
    let street_type = words
        .iter()
        .map(|word| word.trim_end_matches(','))
        .find(|word| STREET_TYPES.contains(&word.to_lowercase().as_str()))
        .unwrap_or("rue");
    let mut parts = Vec::new();
    if has_number {
        parts.push((1 + below(rng, 150)).to_string());
    }
    parts.push(street_type.to_string());
    parts.push(street_name(rng));
    let mut text = parts.join(" ");
    let postal_position = words.iter().position(|word| {
        let word = word.trim_matches(',');
        word.len() == 5 && word.chars().all(|c| c.is_ascii_digit())
    });
    if let Some(position) = postal_position.filter(|position| *position > 0) {
        let separator = if words[position - 1].ends_with(',') {
            ", "
        } else {
            " "
        };
        text.push_str(separator);
        text.push_str(&postal_code(rng));
        text.push(' ');
        text.push_str(pick(rng, &CITIES));
    }
    apply_case(&text, case_of(value))
}

fn pick_in_matching_set<R: Rng + ?Sized>(value: &str, sets: &[[&str; 2]], rng: &mut R) -> String {
    let trimmed = value.trim();
    let set = sets
        .iter()
        .find(|set| {
            set.iter()
                .any(|member| member.to_lowercase() == trimmed.to_lowercase())
        })
        .unwrap_or(&sets[0]);
    apply_case(set[below(rng, 2)], case_of(trimmed))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateLayout {
    Iso,
    French,
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let shifted_month = (month + 9) % 12;
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}

fn parse_date(value: &str) -> Option<(DateLayout, i64, i64, i64, &str)> {
    let head = value.get(..10)?;
    let rest = &value[10..];
    let bytes = head.as_bytes();
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = &head[range];
        if part.bytes().all(|b| b.is_ascii_digit()) {
            part.parse().ok()
        } else {
            None
        }
    };
    let (layout, year, month, day) = if bytes[4] == b'-' && bytes[7] == b'-' {
        (
            DateLayout::Iso,
            number(0..4)?,
            number(5..7)?,
            number(8..10)?,
        )
    } else if bytes[2] == b'/' && bytes[5] == b'/' {
        (
            DateLayout::French,
            number(6..10)?,
            number(3..5)?,
            number(0..2)?,
        )
    } else {
        return None;
    };
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    Some((layout, year, month, day, rest))
}

fn birth_date<R: Rng + ?Sized>(value: &str, rng: &mut R) -> String {
    let weeks = 1 + below(rng, 52) as i64;
    let shift = if below(rng, 2) == 0 { -weeks } else { weeks } * 7;
    match parse_date(value) {
        Some((layout, year, month, day, rest)) => {
            let rest = if rest.is_empty() || TIME_SUFFIX.is_match(rest) {
                rest
            } else {
                ""
            };
            let (year, month, day) = civil_from_days(days_from_civil(year, month, day) + shift);
            match layout {
                DateLayout::Iso => format!("{year:04}-{month:02}-{day:02}{rest}"),
                DateLayout::French => format!("{day:02}/{month:02}/{year:04}{rest}"),
            }
        }
        None => {
            let start = days_from_civil(1940, 1, 1);
            let span = days_from_civil(2005, 12, 31) - start;
            let (year, month, day) = civil_from_days(start + below(rng, span as usize) as i64);
            format!("{year:04}-{month:02}-{day:02}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;

    fn rng(seed: u8) -> ChaCha20Rng {
        ChaCha20Rng::from_seed([seed; 32])
    }

    #[test]
    fn luhn_check_digit_matches_known_numbers() {
        assert_eq!(luhn_check_digit("7992739871"), '3');
        assert_eq!(luhn_check_digit("73282932"), '0');
    }

    #[test]
    fn a_valid_rib_key_yields_the_french_iban_check_76() {
        for seed in 0..50 {
            let text = iban("FR7630004000031234567890143", &mut rng(seed));
            assert!(text.starts_with("FR76"), "{text}");
            let bban = &text[4..];
            let key: u64 = bban[21..].parse().unwrap();
            assert_eq!(key, rib_key(&bban[..5], &bban[5..10], &bban[10..21]));
        }
    }

    #[test]
    fn nir_key_completes_the_body_to_a_multiple_of_97() {
        let text = nir("185057800608436", &mut rng(3));
        let body: u64 = text[..13].parse().unwrap();
        let key: u64 = text[13..].parse().unwrap();
        assert_eq!((body + key) % 97, 0);
        assert!(text.starts_with('1'));
    }

    #[test]
    fn civil_dates_round_trip_through_day_numbers() {
        for days in -40_000..40_000 {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_in_month(2000, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn invalid_dates_are_replaced_by_a_plausible_iso_date() {
        let text = birth_date("1985-02-30", &mut rng(1));
        let (layout, year, ..) = parse_date(&text).unwrap();
        assert_eq!(layout, DateLayout::Iso);
        assert!((1940..=2005).contains(&year));
        assert!(parse_date("not a date").is_none());
    }

    #[test]
    fn birth_dates_keep_a_trailing_time() {
        let text = birth_date("1985-03-12 00:00:00", &mut rng(2));
        assert!(text.ends_with(" 00:00:00"), "{text}");
    }

    #[test]
    fn phone_pool_limit_restricts_the_draws() {
        let config = GeneratorConfig {
            phone_pool_limit: Some(3),
        };
        let mut generator = rng(9);
        for _ in 0..100 {
            let text = phone("0641278395", &mut generator, &config);
            assert!(
                ["0639980000", "0639980001", "0639980002"].contains(&text.as_str()),
                "{text}"
            );
        }
    }

    #[test]
    fn short_phone_inputs_get_a_full_fictional_number() {
        let text = phone("1234", &mut rng(4), &GeneratorConfig::default());
        assert_eq!(text.len(), 14, "{text}");
        assert!(is_fictional_phone(&text), "{text}");
    }

    #[test]
    fn ascii_slug_folds_accents_and_ligatures() {
        assert_eq!(ascii_slug("Maëlouz"), "maelouz");
        assert_eq!(ascii_slug("Jean-Baptiste"), "jean-baptiste");
        assert_eq!(ascii_slug("Œdipe d'Anjou"), "oedipe-d-anjou");
    }

    #[test]
    fn generated_last_names_rarely_collide() {
        let names: std::collections::HashSet<String> = (0..1000u32)
            .map(|n| {
                let mut seed = [0u8; 32];
                seed[..4].copy_from_slice(&n.to_le_bytes());
                last_name(&mut ChaCha20Rng::from_seed(seed))
            })
            .collect();
        assert!(names.len() > 990, "{}", names.len());
    }

    #[test]
    fn civility_and_gender_stay_in_the_input_vocabulary() {
        for seed in 0..20 {
            let civility = pick_in_matching_set("Madame", &CIVILITY_SETS, &mut rng(seed));
            assert!(["Monsieur", "Madame"].contains(&civility.as_str()));
            let gender = pick_in_matching_set("homme", &GENDER_SETS, &mut rng(seed));
            assert!(["homme", "femme"].contains(&gender.as_str()));
            let gender = pick_in_matching_set("H", &GENDER_SETS, &mut rng(seed));
            assert!(["H", "F"].contains(&gender.as_str()));
            let civility = pick_in_matching_set("MME", &CIVILITY_SETS, &mut rng(seed));
            assert!(["M.", "MME"].contains(&civility.as_str()));
        }
    }

    #[test]
    fn addresses_keep_number_street_type_and_postal_layout() {
        let text = address("12 Avenue des Lilas, 75011 Paris", &mut rng(5));
        let words: Vec<&str> = text.split_whitespace().collect();
        assert!(words[0].parse::<u32>().is_ok(), "{text}");
        assert_eq!(words[1], "Avenue", "{text}");
        assert!(text.contains(", "), "{text}");
        let upper = address("3 BD DU MONTPARNASSE", &mut rng(6));
        assert_eq!(upper, upper.to_uppercase());
        assert!(upper.contains(" BD "), "{upper}");
    }

    #[test]
    fn recognisers_accept_only_reserved_namespaces() {
        assert_eq!(recognise("a.b1@EXAMPLE.org"), Some(Kind::Email));
        assert_eq!(recognise("a.b1@example.fr"), None);
        assert_eq!(recognise("0033 7 39 98 00 01"), Some(Kind::Phone));
        assert_eq!(recognise("07 39 97 00 01"), None);
        assert_eq!(recognise("203.0.113.9/32"), Some(Kind::IpAddress));
        assert_eq!(recognise("2001:db9::1"), None);
    }
}
