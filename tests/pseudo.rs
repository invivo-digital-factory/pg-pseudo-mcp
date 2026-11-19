use std::collections::HashSet;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use pg_pseudo_mcp::kind::Kind;
use pg_pseudo_mcp::pseudo::{GeneratorConfig, Pseudonym, Pseudonymizer, purge};

const PHONE_BLOCKS: [&str; 7] = [
    "019900", "026191", "035373", "046571", "053649", "063998", "073998",
];

fn open(dir: &Path) -> Pseudonymizer {
    Pseudonymizer::open(dir).expect("open pseudonymizer")
}

fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

fn luhn_valid(number: &str) -> bool {
    let sum: u32 = number
        .chars()
        .rev()
        .enumerate()
        .map(|(position, c)| {
            let d = c.to_digit(10).expect("digit");
            if position % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum.is_multiple_of(10)
}

fn mod97(numeric: &str) -> u64 {
    numeric.chars().fold(0, |acc, c| {
        (acc * 10 + u64::from(c.to_digit(10).expect("digit"))) % 97
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

fn separator_layout(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_ascii_alphanumeric() { 'x' } else { c })
        .collect()
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn phone_in_fictional_block(text: &str) -> bool {
    let all = digits(text);
    let nsn = &all[all.len() - 9..];
    let national = format!("0{nsn}");
    PHONE_BLOCKS.iter().any(|block| national.starts_with(block))
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn pseudonymizer_can_be_shared_between_threads() {
    assert_send_sync::<Pseudonymizer>();
}

#[test]
fn the_same_email_keeps_its_pseudonym_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let first = open(dir.path()).pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    let second = open(dir.path()).pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    assert_eq!(first, second);
    assert!(first.reversible);
    assert!(!first.exhausted);
    assert_ne!(first.text, "ilona.kervadec@canary-mail.fr");
}

#[test]
fn different_emails_never_share_a_pseudonym() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    let pseudonyms: HashSet<String> = (0..500)
        .map(|n| {
            pseudonymizer
                .pseudonymize(Kind::Email, &format!("person{n}@canary-mail.fr"))
                .text
        })
        .collect();
    assert_eq!(pseudonyms.len(), 500);
}

#[test]
fn email_pseudonyms_live_in_reserved_domains() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    for value in [
        "ilona.kervadec@canary-mail.fr",
        "Maëlys.Brizolier@Canary-Mail.fr",
        "ik.pro@canary-mail.fr",
    ] {
        let text = pseudonymizer.pseudonymize(Kind::Email, value).text;
        let (local, domain) = text.split_once('@').expect("an email");
        assert!(
            ["example.com", "example.net", "example.org"].contains(&domain),
            "{text}"
        );
        assert!(
            local
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-'),
            "{text}"
        );
    }
}

#[test]
fn phone_pseudonyms_use_fictional_blocks_and_keep_the_input_format() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    for value in [
        "06 41 27 83 95",
        "07.58.14.92.36",
        "0652187430",
        "+33 6 41 27 83 95",
        "0033 6 41 27 83 95",
        "01 45 67 89 10",
    ] {
        let text = pseudonymizer.pseudonymize(Kind::Phone, value).text;
        assert_eq!(
            separator_layout(&text),
            separator_layout(value),
            "{value} -> {text}"
        );
        assert!(phone_in_fictional_block(&text), "{value} -> {text}");
    }
    let spaced = pseudonymizer
        .pseudonymize(Kind::Phone, "06 41 27 83 95")
        .text;
    assert!(
        spaced.starts_with("06 39 98 ") || spaced.starts_with("07 39 98 "),
        "{spaced}"
    );
    let landline = pseudonymizer
        .pseudonymize(Kind::Phone, "01 45 67 89 10")
        .text;
    assert!(
        !landline.starts_with("06") && !landline.starts_with("07"),
        "{landline}"
    );
    let bare = pseudonymizer.pseudonymize(Kind::Phone, "33641278395").text;
    assert_eq!(bare.len(), 11, "{bare}");
    assert!(bare.chars().all(|c| c.is_ascii_digit()), "{bare}");
    assert!(
        bare.starts_with("336399") || bare.starts_with("337399"),
        "{bare}"
    );
}

#[test]
fn iban_pseudonyms_pass_mod_97_and_keep_their_grouping() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    for value in [
        "FR76 3000 4000 0312 3456 7890 143",
        "FR14 2004 1010 0505 0001 3M02 606",
        "FR7630004000031234567890143",
    ] {
        let text = pseudonymizer.pseudonymize(Kind::Iban, value).text;
        assert!(iban_valid(&text), "{text}");
        assert_eq!(separator_layout(&text), separator_layout(value), "{text}");
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.starts_with("FR"), "{text}");
        assert_eq!(&compact[4..9], "99999", "{text}");
    }
}

#[test]
fn siren_siret_card_and_nir_pseudonyms_carry_valid_keys() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());

    let siren = pseudonymizer.pseudonymize(Kind::Siren, "418273652").text;
    assert_eq!(siren.len(), 9);
    assert!(luhn_valid(&siren), "{siren}");

    let spaced_siren = pseudonymizer.pseudonymize(Kind::Siren, "418 273 652").text;
    assert_eq!(separator_layout(&spaced_siren), "xxx xxx xxx");
    assert!(luhn_valid(&digits(&spaced_siren)));

    let siret = pseudonymizer
        .pseudonymize(Kind::Siret, "41827365200017")
        .text;
    assert_eq!(siret.len(), 14);
    assert!(luhn_valid(&siret), "{siret}");
    assert!(luhn_valid(&siret[..9]), "{siret}");

    let card = pseudonymizer
        .pseudonymize(Kind::CardNumber, "4970 1012 3456 7890")
        .text;
    assert_eq!(separator_layout(&card), "xxxx xxxx xxxx xxxx");
    let card_digits = digits(&card);
    assert!(card_digits.starts_with("400000"), "{card}");
    assert!(luhn_valid(&card_digits), "{card}");

    let nir = pseudonymizer
        .pseudonymize(Kind::Nir, "2851175123456 22")
        .text;
    assert_eq!(separator_layout(&nir), "xxxxxxxxxxxxx xx");
    let nir_digits = digits(&nir);
    let body: u64 = nir_digits[..13].parse().unwrap();
    let key: u64 = nir_digits[13..].parse().unwrap();
    assert_eq!(key, 97 - body % 97, "{nir}");
    assert_eq!(&nir_digits[..1], "2", "{nir}");
    let month: u32 = nir_digits[3..5].parse().unwrap();
    assert!((1..=12).contains(&month), "{nir}");
}

#[test]
fn ip_pseudonyms_come_from_documentation_ranges() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    let v4 = pseudonymizer
        .pseudonymize(Kind::IpAddress, "82.64.12.7")
        .text;
    assert!(
        ["192.0.2.", "198.51.100.", "203.0.113."]
            .iter()
            .any(|prefix| v4.starts_with(prefix)),
        "{v4}"
    );
    let v6 = pseudonymizer
        .pseudonymize(Kind::IpAddress, "2a01:cb00:1234::1")
        .text;
    assert!(v6.starts_with("2001:db8:"), "{v6}");
}

#[test]
fn reverse_translates_known_pseudonyms_back() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    let cases = [
        (Kind::Email, "ilona.kervadec@canary-mail.fr"),
        (Kind::Phone, "06 41 27 83 95"),
        (Kind::Iban, "FR76 3000 4000 0312 3456 7890 143"),
        (Kind::Siren, "418273652"),
        (Kind::Nir, "2851175123456 22"),
        (Kind::PersonName, "Ilona Kervadec"),
        (Kind::LastName, "Thaumassin"),
        (Kind::IpAddress, "82.64.12.7"),
    ];
    for (kind, value) in cases {
        let pseudonym = pseudonymizer.pseudonymize(kind, value);
        assert!(pseudonym.reversible, "{kind:?}");
        assert_eq!(
            pseudonymizer.reverse(&pseudonym.text),
            Some((kind, value.to_string())),
            "{kind:?}"
        );
    }
    let reopened = open(dir.path());
    let pseudonym = reopened.pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    assert_eq!(
        reopened.reverse(&pseudonym.text),
        Some((Kind::Email, "ilona.kervadec@canary-mail.fr".to_string()))
    );
    assert_eq!(pseudonymizer.reverse("nobody.seen@example.net"), None);
    assert_eq!(pseudonymizer.reverse("ilona.kervadec@canary-mail.fr"), None);
}

#[test]
fn non_reversible_kinds_are_stable_but_never_translated_back() {
    let dir = tempfile::tempdir().unwrap();
    let first = open(dir.path());
    let second = open(dir.path());
    for (kind, value) in [
        (Kind::FirstName, "Ilona"),
        (Kind::City, "Quimper"),
        (Kind::PostalCode, "29000"),
        (Kind::Civility, "Mme"),
        (Kind::Gender, "F"),
        (Kind::Address, "12 rue des Lilas"),
        (Kind::BirthDate, "1985-03-12"),
    ] {
        let a = first.pseudonymize(kind, value);
        let b = second.pseudonymize(kind, value);
        assert_eq!(a, b, "{kind:?}");
        assert!(!a.reversible, "{kind:?}");
        assert_eq!(first.reverse(&a.text), None, "{kind:?}");
    }
}

#[test]
fn looks_like_pseudonym_recognises_reserved_namespaces() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    assert_eq!(
        pseudonymizer.looks_like_pseudonym("nobody.seen42@example.net"),
        Some(Kind::Email)
    );
    assert_eq!(
        pseudonymizer.looks_like_pseudonym("06 39 98 12 34"),
        Some(Kind::Phone)
    );
    assert_eq!(
        pseudonymizer.looks_like_pseudonym("+33 1 99 00 12 34"),
        Some(Kind::Phone)
    );
    assert_eq!(
        pseudonymizer.looks_like_pseudonym("198.51.100.23"),
        Some(Kind::IpAddress)
    );
    assert_eq!(
        pseudonymizer.looks_like_pseudonym("2001:db8::17"),
        Some(Kind::IpAddress)
    );
    let iban = pseudonymizer
        .pseudonymize(Kind::Iban, "FR76 3000 4000 0312 3456 7890 143")
        .text;
    assert_eq!(pseudonymizer.looks_like_pseudonym(&iban), Some(Kind::Iban));
    assert_eq!(
        pseudonymizer.looks_like_pseudonym("ilona.kervadec@canary-mail.fr"),
        None
    );
    assert_eq!(pseudonymizer.looks_like_pseudonym("06 41 27 83 95"), None);
    assert_eq!(pseudonymizer.looks_like_pseudonym("82.64.12.7"), None);
    assert_eq!(pseudonymizer.looks_like_pseudonym("hello"), None);
}

#[test]
fn the_store_holds_no_real_value_in_clear() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    let mut values = Vec::new();
    for n in 0..25 {
        values.push((Kind::Email, format!("canary.person{n:03}@canary-mail.fr")));
        values.push((Kind::Phone, format!("06 41 27 {:02} {:02}", n / 10, n)));
        values.push((Kind::LastName, format!("Kervadecanary{n:03}")));
        values.push((Kind::PersonName, format!("Ilona Thaumassinary{n:03}")));
    }
    for (kind, value) in &values {
        pseudonymizer.pseudonymize(*kind, value);
    }
    let mut bytes = std::fs::read(dir.path().join("mapping.sqlite")).unwrap();
    if let Ok(wal) = std::fs::read(dir.path().join("mapping.sqlite-wal")) {
        bytes.extend(wal);
    }
    for (_, value) in &values {
        let needle = value.as_bytes();
        assert!(
            !bytes.windows(needle.len()).any(|window| window == needle),
            "{value} found in clear"
        );
    }
    assert!(!bytes.windows(13).any(|window| window == b"canary-mail.f"));
}

#[test]
fn secret_and_store_are_private_to_the_user() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    pseudonymizer.pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    for name in ["secret", "mapping.sqlite", "mapping.sqlite-wal"] {
        let path = dir.path().join(name);
        if name.ends_with("-wal") && !path.exists() {
            continue;
        }
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{name}");
    }
}

#[test]
fn concurrent_openers_share_one_secret() {
    let dir = tempfile::tempdir().unwrap();
    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let path = dir.path().to_path_buf();
            std::thread::spawn(move || {
                barrier.wait();
                Pseudonymizer::open(&path)
                    .expect("open")
                    .pseudonymize(Kind::FirstName, "Ilona")
                    .text
            })
        })
        .collect();
    let results: HashSet<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.len(), 1, "{results:?}");
    assert_eq!(std::fs::read(dir.path().join("secret")).unwrap().len(), 32);
}

#[test]
fn a_store_without_its_secret_points_to_the_purge_command() {
    let dir = tempfile::tempdir().unwrap();
    open(dir.path()).pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    std::fs::remove_file(dir.path().join("secret")).unwrap();
    let error = match Pseudonymizer::open(dir.path()) {
        Ok(_) => panic!("open must fail without the secret"),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("pg-pseudo-mcp mapping purge"), "{error}");
}

#[test]
fn purge_deletes_the_store_and_the_secret() {
    let dir = tempfile::tempdir().unwrap();
    let before = open(dir.path()).pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    let deleted = purge(dir.path()).unwrap();
    assert!(deleted.contains(&dir.path().join("secret")));
    assert!(deleted.contains(&dir.path().join("mapping.sqlite")));
    for path in &deleted {
        assert!(!path.exists(), "{}", path.display());
    }
    let after = open(dir.path());
    assert_eq!(after.reverse(&before.text), None);
}

#[test]
fn mapping_purge_command_asks_for_confirmation() {
    let dir = tempfile::tempdir().unwrap();
    open(dir.path()).pseudonymize(Kind::Email, "ilona.kervadec@canary-mail.fr");
    let binary = env!("CARGO_BIN_EXE_pg-pseudo-mcp");

    let mut refused = Command::new(binary)
        .args(["mapping", "purge"])
        .env("PG_PSEUDO_DATA_DIR", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    refused.stdin.take().unwrap().write_all(b"no\n").unwrap();
    let refused = refused.wait_with_output().unwrap();
    assert!(!refused.status.success());
    assert!(dir.path().join("secret").exists());

    let mut confirmed = Command::new(binary)
        .args(["mapping", "purge"])
        .env("PG_PSEUDO_DATA_DIR", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    confirmed.stdin.take().unwrap().write_all(b"yes\n").unwrap();
    let confirmed = confirmed.wait_with_output().unwrap();
    assert!(confirmed.status.success());
    let stderr = String::from_utf8_lossy(&confirmed.stderr);
    assert!(stderr.contains("mapping.sqlite"), "{stderr}");
    assert!(!dir.path().join("secret").exists());
    assert!(!dir.path().join("mapping.sqlite").exists());

    open(dir.path());
    let forced = Command::new(binary)
        .args(["mapping", "purge", "--yes"])
        .env("PG_PSEUDO_DATA_DIR", dir.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(forced.status.success());
    assert!(!dir.path().join("secret").exists());
}

#[test]
fn birth_dates_shift_by_whole_weeks_and_keep_their_format() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    for n in 0..40 {
        let day = 1 + n % 28;
        let iso = format!("1985-03-{day:02}");
        let shifted = pseudonymizer.pseudonymize(Kind::BirthDate, &iso).text;
        assert_eq!(separator_layout(&shifted), "xxxx-xx-xx", "{shifted}");
        let parse = |text: &str| {
            days_from_civil(
                text[..4].parse().unwrap(),
                text[5..7].parse().unwrap(),
                text[8..10].parse().unwrap(),
            )
        };
        let delta = parse(&shifted) - parse(&iso);
        assert_eq!(delta % 7, 0, "{iso} -> {shifted}");
        assert!((1..=52).contains(&(delta.abs() / 7)), "{iso} -> {shifted}");

        let french = format!("{day:02}/03/1985");
        let shifted = pseudonymizer.pseudonymize(Kind::BirthDate, &french).text;
        assert_eq!(separator_layout(&shifted), "xx/xx/xxxx", "{shifted}");
        let delta = days_from_civil(
            shifted[6..10].parse().unwrap(),
            shifted[3..5].parse().unwrap(),
            shifted[..2].parse().unwrap(),
        ) - days_from_civil(1985, 3, i64::from(day));
        assert_eq!(delta % 7, 0, "{french} -> {shifted}");
        assert!(
            (1..=52).contains(&(delta.abs() / 7)),
            "{french} -> {shifted}"
        );
    }
}

#[test]
fn a_single_number_phone_pool_exhausts_on_the_second_phone() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = Pseudonymizer::open_with_config(
        dir.path(),
        GeneratorConfig {
            phone_pool_limit: Some(1),
        },
    )
    .unwrap();
    let first = pseudonymizer.pseudonymize(Kind::Phone, "06 41 27 83 95");
    assert!(!first.exhausted);
    let started = Instant::now();
    let second = pseudonymizer.pseudonymize(Kind::Phone, "06 41 27 83 96");
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(second.exhausted);
    assert!(!second.reversible);
    assert_opaque_phone_token(&second);
    assert_eq!(
        pseudonymizer.pseudonymize(Kind::Phone, "06 41 27 83 96"),
        second
    );
}

#[test]
fn a_ten_number_phone_pool_stays_injective_then_exhausts() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = Pseudonymizer::open_with_config(
        dir.path(),
        GeneratorConfig {
            phone_pool_limit: Some(10),
        },
    )
    .unwrap();
    let started = Instant::now();
    let results: Vec<(String, Pseudonym)> = (0..11)
        .map(|n| {
            let value = format!("06 41 27 83 {n:02}");
            let pseudonym = pseudonymizer.pseudonymize(Kind::Phone, &value);
            (value, pseudonym)
        })
        .collect();
    assert!(started.elapsed() < Duration::from_secs(5));
    let granted: Vec<&(String, Pseudonym)> = results.iter().filter(|(_, p)| !p.exhausted).collect();
    let distinct: HashSet<&str> = granted.iter().map(|(_, p)| p.text.as_str()).collect();
    assert_eq!(distinct.len(), granted.len());
    assert!(granted.len() <= 10);
    for (value, pseudonym) in &granted {
        assert_eq!(
            pseudonymizer.reverse(&pseudonym.text),
            Some((Kind::Phone, value.clone()))
        );
    }
    for (_, pseudonym) in results.iter().filter(|(_, p)| p.exhausted) {
        assert_opaque_phone_token(pseudonym);
    }
    if results[..10].iter().all(|(_, p)| !p.exhausted) {
        assert!(results[10].1.exhausted);
    }
}

fn assert_opaque_phone_token(pseudonym: &Pseudonym) {
    let text = &pseudonym.text;
    let id = text
        .strip_prefix("<phone id=")
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or_else(|| panic!("{text}"));
    assert_eq!(id.len(), 8, "{text}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{text}");
}

#[test]
fn person_names_keep_the_case_pattern_of_the_input() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());

    let title = pseudonymizer
        .pseudonymize(Kind::PersonName, "Ilona Kervadec")
        .text;
    let words: Vec<&str> = title.split(' ').collect();
    assert_eq!(words.len(), 2, "{title}");
    for word in &words {
        let mut chars = word.chars();
        assert!(chars.next().unwrap().is_uppercase(), "{title}");
        assert!(chars.any(char::is_lowercase), "{title}");
    }

    let upper = pseudonymizer
        .pseudonymize(Kind::PersonName, "ODILON THAUMASSIN")
        .text;
    assert_eq!(upper, upper.to_uppercase());
    assert_eq!(upper.split(' ').count(), 2, "{upper}");

    let lower = pseudonymizer
        .pseudonymize(Kind::PersonName, "maëlys brizolier")
        .text;
    assert_eq!(lower, lower.to_lowercase());

    let mixed = pseudonymizer
        .pseudonymize(Kind::PersonName, "Jean DUPONT")
        .text;
    let (first, last) = mixed.split_once(' ').unwrap();
    assert_ne!(first, first.to_uppercase(), "{mixed}");
    assert_eq!(last, last.to_uppercase(), "{mixed}");

    let last_name = pseudonymizer.pseudonymize(Kind::LastName, "KERVADEC").text;
    assert_eq!(last_name, last_name.to_uppercase());
    let first_name = pseudonymizer.pseudonymize(Kind::FirstName, "ilona").text;
    assert_eq!(first_name, first_name.to_lowercase());
}

#[test]
fn last_names_are_drawn_from_a_large_space() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    let pseudonyms: HashSet<String> = (0..1000)
        .map(|n| pseudonymizer.pseudonymize(Kind::LastName, &format!("Kervadec{n}")))
        .filter(|p| !p.exhausted)
        .map(|p| p.text)
        .collect();
    assert!(pseudonyms.len() > 990, "{}", pseudonyms.len());
}

#[test]
fn empty_values_pass_through() {
    let dir = tempfile::tempdir().unwrap();
    let pseudonymizer = open(dir.path());
    let pseudonym = pseudonymizer.pseudonymize(Kind::Email, "");
    assert_eq!(pseudonym.text, "");
    assert!(!pseudonym.reversible);
}
