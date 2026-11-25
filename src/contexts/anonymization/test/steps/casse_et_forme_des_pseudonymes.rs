use cucumber::then;

use crate::anonymization_world::{AnonymizationWorld, layout, listed};

fn words(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

fn is_upper(word: &str) -> bool {
    word.chars().any(char::is_alphabetic) && word == word.to_uppercase()
}

fn is_lower(word: &str) -> bool {
    word.chars().any(char::is_alphabetic) && word == word.to_lowercase()
}

fn is_title(word: &str) -> bool {
    word.split(['-', ' ', '\''])
        .filter(|part| !part.is_empty())
        .all(|part| {
            let mut chars = part.chars();
            chars.next().is_some_and(char::is_uppercase)
                && (part.chars().count() == 1 || chars.any(char::is_lowercase))
        })
}

fn alphabetic_words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|word| word.chars().count() >= 3)
        .map(str::to_lowercase)
        .collect()
}

#[then(regex = r"^le pseudonyme est en deux mots en casse titre$")]
async fn two_title_words(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let parts = words(pseudonym);
    assert_eq!(parts.len(), 2, "{pseudonym}");
    assert!(parts.iter().all(|word| is_title(word)), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en deux mots en majuscules$")]
async fn two_upper_words(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let parts = words(pseudonym);
    assert_eq!(parts.len(), 2, "{pseudonym}");
    assert!(parts.iter().all(|word| is_upper(word)), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en deux mots en minuscules$")]
async fn two_lower_words(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let parts = words(pseudonym);
    assert_eq!(parts.len(), 2, "{pseudonym}");
    assert!(parts.iter().all(|word| is_lower(word)), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est un prénom en casse titre et un nom en majuscules$")]
async fn title_then_upper(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    let (first, last) = pseudonym.rsplit_once(' ').expect("two words");
    assert!(is_title(first), "{pseudonym}");
    assert!(is_upper(last), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en un mot en majuscules$")]
async fn one_upper_word(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(words(pseudonym).len(), 1, "{pseudonym}");
    assert!(is_upper(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en un mot en casse titre$")]
async fn one_title_word(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(words(pseudonym).len(), 1, "{pseudonym}");
    assert!(is_title(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en minuscules$")]
async fn lower_case(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert!(is_lower(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en majuscules$")]
async fn upper_case(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert!(is_upper(pseudonym), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est en casse titre$")]
async fn title_case(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym();
    assert!(is_title(pseudonym), "{pseudonym}");
}

#[then(regex = r#"^le pseudonyme a la présentation "([^"]+)"$"#)]
async fn pseudonym_has_layout(world: &mut AnonymizationWorld, expected: String) {
    let pseudonym = world.last_pseudonym();
    assert_eq!(layout(pseudonym), expected, "{pseudonym}");
}

#[then(regex = r#"^le pseudonyme commence par l'un de "([^"]+)"$"#)]
async fn pseudonym_starts_with_one_of(world: &mut AnonymizationWorld, prefixes: String) {
    let pseudonym = world.last_pseudonym();
    assert!(
        listed(&prefixes)
            .iter()
            .any(|prefix| pseudonym.starts_with(prefix.as_str())),
        "{pseudonym} does not start with one of {prefixes}"
    );
}

#[then(regex = r"^le pseudonyme ne contient aucun mot de l'original$")]
async fn pseudonym_copies_no_word(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym().to_lowercase();
    for word in alphabetic_words(world.input()) {
        assert!(!pseudonym.contains(&word), "{word} copied into {pseudonym}");
    }
}

#[then(regex = r#"^le pseudonyme se termine par "([^"]+)"$"#)]
async fn pseudonym_ends_with(world: &mut AnonymizationWorld, suffix: String) {
    let pseudonym = world.last_pseudonym();
    assert!(pseudonym.ends_with(&suffix), "{pseudonym}");
}
