use cucumber::{given, then, when};

use crate::anonymization_world::AnonymizationWorld;

const RESERVED_DOMAINS: [&str; 3] = ["example.com", "example.net", "example.org"];

#[given(regex = r"^un répertoire de données neuf$")]
async fn fresh_data_directory(world: &mut AnonymizationWorld) {
    world.pseudonyms.clear();
    world.translation = None;
    assert!(
        std::fs::read_dir(world.data_dir.path())
            .expect("data dir")
            .next()
            .is_none()
    );
}

#[when(regex = r#"^l'agent obtient deux fois le pseudonyme de l'adresse "([^"]+)"$"#)]
async fn agent_gets_pseudonym_twice(world: &mut AnonymizationWorld, email: String) {
    let anonymize = world.use_cases().anonymize_free_text;
    let first = anonymize.execute(&email, &[]);
    let second = anonymize.execute(&email, &[]);
    world.pseudonyms = vec![first, second];
}

#[when(regex = r#"^l'agent obtient le pseudonyme de l'adresse "([^"]+)"$"#)]
async fn agent_gets_pseudonym(world: &mut AnonymizationWorld, email: String) {
    let pseudonym = world.use_cases().anonymize_free_text.execute(&email, &[]);
    world.pseudonyms = vec![pseudonym];
}

#[when(regex = r"^l'agent traduit ce pseudonyme$")]
async fn agent_translates_pseudonym(world: &mut AnonymizationWorld) {
    let pseudonym = world.pseudonyms.first().expect("a pseudonym was issued");
    let translation = world.use_cases().translate_pseudonym.execute(pseudonym);
    world.translation = Some(translation.real.map(|(_, real)| real));
}

#[when(regex = r"^le développeur purge la table de correspondance$")]
async fn developer_purges_mapping(world: &mut AnonymizationWorld) {
    world.purge();
}

#[then(regex = r"^les deux pseudonymes sont identiques$")]
async fn pseudonyms_are_identical(world: &mut AnonymizationWorld) {
    assert_eq!(world.pseudonyms.len(), 2);
    assert_eq!(world.pseudonyms[0], world.pseudonyms[1]);
}

#[then(regex = r"^le pseudonyme n'est pas l'adresse réelle$")]
async fn pseudonym_is_not_the_real_address(world: &mut AnonymizationWorld) {
    let pseudonym = &world.pseudonyms[0];
    assert!(!pseudonym.contains("canary-mail"), "{pseudonym}");
    assert!(!pseudonym.contains("kervadec"), "{pseudonym}");
}

#[then(regex = r"^le pseudonyme est sur un domaine réservé$")]
async fn pseudonym_uses_a_reserved_domain(world: &mut AnonymizationWorld) {
    let pseudonym = &world.pseudonyms[0];
    let domain = pseudonym.rsplit('@').next().expect("an address");
    assert!(RESERVED_DOMAINS.contains(&domain), "{pseudonym}");
}

#[then(regex = r#"^la traduction rend l'adresse "([^"]+)"$"#)]
async fn translation_returns_the_address(world: &mut AnonymizationWorld, email: String) {
    assert_eq!(world.translation, Some(Some(email)));
}

#[then(regex = r"^la traduction ne rend aucune valeur réelle$")]
async fn translation_returns_nothing(world: &mut AnonymizationWorld) {
    assert_eq!(world.translation, Some(None));
}
