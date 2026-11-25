use std::os::unix::fs::PermissionsExt;

use cucumber::{then, when};
use scarif_database_mcp::contexts::anonymization::domain::types::GeneratorConfig;
use scarif_database_mcp::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;

use crate::anonymization_world::{AnonymizationWorld, kind_named, listed};

const SECRET: &str = "secret";
const STORE_FILES: [&str; 4] = [
    "mapping.sqlite",
    "mapping.sqlite-wal",
    "mapping.sqlite-shm",
    "secret",
];

fn canary_value(kind: &str, n: usize) -> String {
    match kind {
        "email" => format!("canary.person{n:03}@canary-mail.fr"),
        "phone" => format!("06 41 27 {:02} {:02}", n / 10, n),
        "last_name" => format!("Kervadecanary{n:03}"),
        "person_name" => format!("Ylwenna Hazvekmorary{n:03}"),
        other => panic!("no canary values for {other}"),
    }
}

#[when(
    regex = r#"^l'agent obtient le pseudonyme de (\d+) valeurs canari pour chacun des types "([^"]+)"$"#
)]
async fn agent_pseudonymizes_canaries(world: &mut AnonymizationWorld, count: usize, kinds: String) {
    let pseudonymizer = world.pseudonymizer();
    for kind in listed(&kinds) {
        for n in 0..count {
            let value = canary_value(&kind, n);
            pseudonymizer.pseudonymize(kind_named(&kind), &value);
            world.canaries.push(value);
        }
    }
}

#[when(regex = r"^le secret du répertoire de données est supprimé$")]
async fn secret_is_deleted(world: &mut AnonymizationWorld) {
    std::fs::remove_file(world.data_dir.path().join(SECRET)).expect("secret removed");
}

#[when(regex = r"^le secret du second poste remplace celui du répertoire de données$")]
async fn foreign_secret_replaces(world: &mut AnonymizationWorld) {
    let other = world.other_data_dir.as_ref().expect("a second workstation");
    open_pseudonymizer(other.path(), GeneratorConfig::default()).expect("second store opens");
    std::fs::copy(
        other.path().join(SECRET),
        world.data_dir.path().join(SECRET),
    )
    .expect("secret copied");
}

#[when(regex = r"^le serveur ouvre le répertoire de données$")]
async fn server_opens_data_directory(world: &mut AnonymizationWorld) {
    world.open_error = world.use_cases_or_error().err();
}

#[then(regex = r"^aucune de ces valeurs n'apparaît en clair dans les fichiers du magasin$")]
async fn no_clear_value_in_store(world: &mut AnonymizationWorld) {
    let mut bytes = Vec::new();
    for name in STORE_FILES.iter().filter(|name| **name != SECRET) {
        if let Ok(content) = std::fs::read(world.data_dir.path().join(name)) {
            bytes.extend(content);
        }
    }
    assert!(!bytes.is_empty());
    assert!(!world.canaries.is_empty());
    for value in &world.canaries {
        let needle = value.as_bytes();
        assert!(
            !bytes.windows(needle.len()).any(|window| window == needle),
            "{value} found in clear"
        );
    }
    assert!(!bytes.windows(11).any(|window| window == b"canary-mail"));
}

#[then(regex = r#"^les fichiers "([^"]+)" du répertoire de données ont les droits 600$"#)]
async fn files_are_private(world: &mut AnonymizationWorld, names: String) {
    for name in listed(&names) {
        let path = world.data_dir.path().join(&name);
        let mode = std::fs::metadata(&path)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "{name}");
    }
}

#[then(regex = r"^le secret fait (\d+) octets$")]
async fn secret_length(world: &mut AnonymizationWorld, length: usize) {
    let secret = std::fs::read(world.data_dir.path().join(SECRET)).expect("secret");
    assert_eq!(secret.len(), length);
}

#[then(regex = r#"^l'ouverture échoue en conseillant "([^"]+)"$"#)]
async fn opening_fails_with_advice(world: &mut AnonymizationWorld, advice: String) {
    let error = world.open_error.as_deref().expect("opening failed");
    assert!(error.contains(&advice), "{error}");
}

#[then(regex = r"^le répertoire de données ne contient plus ni magasin, ni journal, ni secret$")]
async fn data_directory_is_purged(world: &mut AnonymizationWorld) {
    for name in STORE_FILES {
        assert!(!world.data_dir.path().join(name).exists(), "{name}");
    }
}
