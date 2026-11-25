use cucumber::{given, then, when};
use scarif_database_mcp::common::AppConfig;
use scarif_database_mcp::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use scarif_database_mcp::contexts::anonymization::infrastructure::data_sources::data_directory::{
    DataDirectory, open_pseudonymizer, resolve_data_dir,
};

use crate::anonymization_world::AnonymizationWorld;

const LEGACY_DIRECTORY: &str = "pg-pseudo-mcp";
const KNOWN_EMAIL: &str = "ylwenna.kervadec@canary-mail.fr";

fn root(world: &AnonymizationWorld) -> &std::path::Path {
    world.data_home.as_ref().expect("a data root").path()
}

fn resolved(world: &AnonymizationWorld) -> &DataDirectory {
    world
        .resolved
        .as_ref()
        .expect("the data directory was resolved")
        .as_ref()
        .unwrap_or_else(|error| panic!("{error}"))
}

fn known_pseudonym(directory: &std::path::Path) -> String {
    open_pseudonymizer(directory, GeneratorConfig::default())
        .expect("store opens")
        .pseudonymize(Kind::Email, KNOWN_EMAIL)
        .text
}

#[given(regex = r"^une racine de données$")]
async fn data_root(world: &mut AnonymizationWorld) {
    world.data_home = Some(tempfile::tempdir().expect("data root"));
}

#[given(regex = r#"^le dossier "([^"]+)" porte un secret$"#)]
async fn directory_holds_a_secret(world: &mut AnonymizationWorld, name: String) {
    let pseudonym = known_pseudonym(&root(world).join(&name));
    if name == LEGACY_DIRECTORY {
        world.legacy_pseudonym = Some(pseudonym);
    }
}

#[given(regex = r#"^le dossier "([^"]+)" existe sans secret$"#)]
async fn directory_exists_without_secret(world: &mut AnonymizationWorld, name: String) {
    std::fs::create_dir(root(world).join(name)).expect("directory created");
}

#[given(regex = r#"^un répertoire de données explicite "([^"]+)"$"#)]
async fn explicit_data_directory(world: &mut AnonymizationWorld, name: String) {
    world.explicit_dir = Some(root(world).join(name));
}

#[when(regex = r"^le serveur choisit son répertoire de données$")]
async fn server_resolves_data_directory(world: &mut AnonymizationWorld) {
    let config = AppConfig {
        data_dir: world.explicit_dir.clone(),
        data_home: world
            .data_home
            .as_ref()
            .map(|home| home.path().to_path_buf()),
        ..AppConfig::default()
    };
    world.resolved = Some(resolve_data_dir(&config).map_err(|error| error.to_string()));
}

#[then(regex = r#"^il utilise le dossier "([^"]+)"$"#)]
async fn uses_directory(world: &mut AnonymizationWorld, name: String) {
    assert_eq!(resolved(world).path, root(world).join(name));
}

#[then(regex = r"^il signale un conflit$")]
async fn reports_conflict(world: &mut AnonymizationWorld) {
    assert!(resolved(world).conflict);
}

#[then(regex = r"^il ne signale aucun conflit$")]
async fn reports_no_conflict(world: &mut AnonymizationWorld) {
    assert!(!resolved(world).conflict);
}

#[then(regex = r#"^le dossier "([^"]+)" n'existe pas$"#)]
async fn directory_does_not_exist(world: &mut AnonymizationWorld, name: String) {
    assert!(!root(world).join(name).exists());
}

#[then(regex = r"^un pseudonyme émis dans l'ancien dossier reste identique$")]
async fn legacy_pseudonym_is_kept(world: &mut AnonymizationWorld) {
    let expected = world.legacy_pseudonym.clone().expect("a legacy pseudonym");
    assert_eq!(known_pseudonym(&resolved(world).path), expected);
}

#[then(regex = r"^aucun répertoire de données n'est choisi$")]
async fn no_data_directory(world: &mut AnonymizationWorld) {
    let outcome = world
        .resolved
        .as_ref()
        .expect("the data directory was resolved");
    assert!(outcome.is_err(), "{outcome:?}");
}
