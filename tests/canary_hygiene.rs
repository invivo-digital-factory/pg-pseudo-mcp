mod common;

use fake::locales::{Data, FR_FR};

fn words_in_name_pools(values: &[&str]) -> Vec<String> {
    let pool: Vec<String> = FR_FR::NAME_FIRST_NAME
        .iter()
        .chain(FR_FR::NAME_LAST_NAME.iter())
        .map(|name| name.to_lowercase())
        .collect();
    let mut found = Vec::new();
    for value in values {
        for word in value.split(|c: char| !c.is_alphabetic()) {
            let word = word.to_lowercase();
            if word.chars().count() >= 3 && pool.contains(&word) && !found.contains(&word) {
                found.push(word);
            }
        }
    }
    found
}

#[test]
fn canary_values_never_share_a_word_with_the_pseudonym_name_pools() {
    let shared = words_in_name_pools(common::CANARY_VALUES);
    assert!(
        shared.is_empty(),
        "canary words {shared:?} are in the name lists the generator draws pseudonyms from; \
         a pseudonym can then contain them (flaky leak checks) and name detection masks \
         leaks that rare real names would reveal; use invented names absent from those lists"
    );
}

#[test]
fn the_hygiene_check_catches_a_common_first_name() {
    assert_eq!(
        words_in_name_pools(&["Marie Kervadec", "marie.k@canary-mail.fr"]),
        vec!["marie".to_string()]
    );
}
