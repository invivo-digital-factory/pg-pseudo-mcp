use cucumber::{then, when};

use crate::anonymization_world::{AnonymizationWorld, kind_named};

#[when(regex = r#"^le dictionnaire lit le nom de colonne "([^"]+)"$"#)]
async fn dictionary_reads_column_name(world: &mut AnonymizationWorld, column: String) {
    world.dictionary = Some(world.use_cases().dictionary_kind.execute(&column));
}

#[then(regex = r#"^le dictionnaire y reconnaît le type "([^"]+)"$"#)]
async fn dictionary_recognises_kind(world: &mut AnonymizationWorld, kind: String) {
    assert_eq!(world.dictionary, Some(Some(kind_named(&kind))));
}

#[then(regex = r"^le dictionnaire n'y reconnaît aucun type$")]
async fn dictionary_recognises_nothing(world: &mut AnonymizationWorld) {
    assert_eq!(world.dictionary, Some(None));
}
