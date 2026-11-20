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

mod unit {
    use super::*;

    #[test]
    fn les_valeurs_canari_ne_partagent_aucun_mot_avec_les_noms_des_pseudonymes() {
        let shared = words_in_name_pools(common::CANARY_VALUES);
        assert!(
            shared.is_empty(),
            "canary words {shared:?} are in the name lists the generator draws pseudonyms from; \
             a pseudonym can then contain them (flaky leak checks) and name detection masks \
             leaks that rare real names would reveal; use invented names absent from those lists"
        );
    }

    #[test]
    fn le_controle_d_hygiene_detecte_un_prenom_courant() {
        assert_eq!(
            words_in_name_pools(&["Marie Kervadec", "marie.k@canary-mail.fr"]),
            vec!["marie".to_string()]
        );
    }
}
