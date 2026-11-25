use std::collections::HashSet;

use cucumber::{given, then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use scarif_database_mcp::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;

use crate::anonymization_world::{AnonymizationWorld, kind_named};

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

#[given(regex = r"^un second poste avec son propre répertoire de données$")]
async fn second_workstation(world: &mut AnonymizationWorld) {
    world.other_data_dir = Some(tempfile::tempdir().expect("second data dir"));
}

#[given(regex = r#"^l'agent a filtré sur le pseudonyme de type "([^"]+)" de "([^"]+)"$"#)]
async fn agent_filtered_on_pseudonym(world: &mut AnonymizationWorld, kind: String, real: String) {
    let pseudonym = world
        .pseudonymizer()
        .pseudonymize(kind_named(&kind), &real)
        .text;
    world.pseudonyms.push(pseudonym.clone());
    world.injected.push((real, pseudonym));
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

#[when(regex = r#"^l'agent obtient le pseudonyme de type "([^"]+)" de "([^"]*)"$"#)]
async fn agent_gets_pseudonym_of_kind(world: &mut AnonymizationWorld, kind: String, value: String) {
    let pseudonym = world
        .pseudonymizer()
        .pseudonymize(kind_named(&kind), &value)
        .text;
    world.input = Some(value);
    world.pseudonyms.push(pseudonym);
}

#[when(
    regex = r#"^le serveur démarre deux fois et donne chaque fois le pseudonyme de type "([^"]+)" de "([^"]+)"$"#
)]
async fn server_restarts(world: &mut AnonymizationWorld, kind: String, value: String) {
    let kind = kind_named(&kind);
    let first = world.pseudonymizer().pseudonymize(kind, &value).text;
    let second = world.pseudonymizer().pseudonymize(kind, &value).text;
    world.pseudonyms = vec![first, second];
}

#[when(regex = r#"^chaque poste donne le pseudonyme de type "([^"]+)" de "([^"]+)"$"#)]
async fn each_workstation_pseudonymizes(
    world: &mut AnonymizationWorld,
    kind: String,
    value: String,
) {
    let kind = kind_named(&kind);
    let other = world.other_data_dir.as_ref().expect("a second workstation");
    let elsewhere = open_pseudonymizer(other.path(), GeneratorConfig::default())
        .expect("second store opens")
        .pseudonymize(kind, &value)
        .text;
    let here = world.pseudonymizer().pseudonymize(kind, &value).text;
    world.pseudonyms = vec![here, elsewhere];
}

#[when(regex = r"^l'agent obtient le pseudonyme de (\d+) adresses e-mail canari différentes$")]
async fn agent_gets_many_pseudonyms(world: &mut AnonymizationWorld, count: usize) {
    let pseudonymizer = world.pseudonymizer();
    world.pseudonyms = (0..count)
        .map(|n| {
            pseudonymizer
                .pseudonymize(Kind::Email, &format!("canary.person{n:03}@canary-mail.fr"))
                .text
        })
        .collect();
}

#[when(regex = r"^l'agent traduit ce pseudonyme$")]
async fn agent_translates_pseudonym(world: &mut AnonymizationWorld) {
    let pseudonym = world.last_pseudonym().to_string();
    let translation = world.use_cases().translate_pseudonym.execute(&pseudonym);
    world.translation = Some(translation.real.map(|(_, real)| real));
}

#[when(regex = r#"^l'agent traduit "([^"]+)"$"#)]
async fn agent_translates_text(world: &mut AnonymizationWorld, text: String) {
    let translation = world.use_cases().translate_pseudonym.execute(&text);
    world.translation = Some(translation.real.map(|(_, real)| real));
    world.recognised = Some(translation.recognised);
}

#[when(regex = r"^le développeur purge la table de correspondance$")]
async fn developer_purges_mapping(world: &mut AnonymizationWorld) {
    world.purge();
}

#[when(regex = r#"^l'agent lit le texte libre "([^"]*)"$"#)]
async fn agent_reads_free_text(world: &mut AnonymizationWorld, text: String) {
    let rendered = world
        .use_cases()
        .anonymize_free_text
        .execute(&text, &world.injected);
    world.input = Some(text);
    world.pseudonyms.push(rendered);
}

#[then(regex = r"^les deux pseudonymes sont identiques$")]
async fn pseudonyms_are_identical(world: &mut AnonymizationWorld) {
    assert_eq!(world.pseudonyms.len(), 2);
    assert_eq!(world.pseudonyms[0], world.pseudonyms[1]);
}

#[then(regex = r"^les deux pseudonymes sont différents$")]
async fn pseudonyms_differ(world: &mut AnonymizationWorld) {
    assert_eq!(world.pseudonyms.len(), 2);
    assert_ne!(world.pseudonyms[0], world.pseudonyms[1]);
}

#[then(regex = r"^les (\d+) pseudonymes sont tous différents$")]
async fn pseudonyms_all_differ(world: &mut AnonymizationWorld, count: usize) {
    let distinct: HashSet<&String> = world.pseudonyms.iter().collect();
    assert_eq!(world.pseudonyms.len(), count);
    assert_eq!(distinct.len(), count);
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

#[then(regex = r"^le pseudonyme se traduit en retour : (oui|non)$")]
async fn pseudonym_translates_back(world: &mut AnonymizationWorld, answer: String) {
    let expected = (answer == "oui").then(|| world.input().to_string());
    assert_eq!(world.translation, Some(expected));
}

#[then(regex = r#"^la forme reconnue est "([^"]+)"$"#)]
async fn recognised_shape_is(world: &mut AnonymizationWorld, kind: String) {
    let expected = (kind != "aucun").then(|| kind_named(&kind));
    assert_eq!(world.recognised, Some(expected));
}

#[then(regex = r#"^la valeur rendue est "([^"]*)"$"#)]
async fn rendered_value_is(world: &mut AnonymizationWorld, expected: String) {
    assert_eq!(world.last_pseudonym(), world.expand(&expected));
}

#[then(regex = r#"^la valeur rendue ne contient plus "([^"]+)"$"#)]
async fn rendered_value_lacks(world: &mut AnonymizationWorld, fragment: String) {
    let rendered = world.last_pseudonym();
    for form in [
        fragment.clone(),
        fragment.to_lowercase(),
        fragment.to_uppercase(),
    ] {
        assert!(!rendered.contains(&form), "{form} leaked in {rendered}");
    }
}

#[then(regex = r#"^la valeur rendue contient "([^"]+)"$"#)]
async fn rendered_value_contains(world: &mut AnonymizationWorld, fragment: String) {
    let fragment = world.expand(&fragment);
    let rendered = world.last_pseudonym();
    assert!(
        rendered.contains(&fragment),
        "{fragment} missing in {rendered}"
    );
}

#[then(regex = r#"^la valeur rendue contient un jeton opaque "([^"]+)"$"#)]
async fn rendered_value_contains_token(world: &mut AnonymizationWorld, label: String) {
    let rendered = world.last_pseudonym();
    assert!(rendered.contains(&format!("<{label} id=")), "{rendered}");
}

#[then(regex = r#"^la valeur rendue est un jeton opaque "([^"]+)"$"#)]
async fn rendered_value_is_token(world: &mut AnonymizationWorld, label: String) {
    let rendered = world.last_pseudonym();
    let id = rendered
        .strip_prefix(&format!("<{label} id="))
        .and_then(|rest| rest.strip_suffix('>'))
        .unwrap_or_else(|| panic!("{rendered} is not a {label} token"));
    assert_eq!(id.len(), 8, "{rendered}");
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()), "{rendered}");
}

#[then(regex = r"^le pseudonyme est vide$")]
async fn pseudonym_is_empty(world: &mut AnonymizationWorld) {
    assert_eq!(world.last_pseudonym(), "");
}
