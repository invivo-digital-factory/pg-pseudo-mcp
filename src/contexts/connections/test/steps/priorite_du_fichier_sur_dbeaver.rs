use cucumber::{given, then};
use scarif_database_mcp::contexts::connections::domain::repositories::ConnectionCatalog;
use serde_json::{Value, json};

use crate::connections_steps::support::{connections, entries, write_connection_file};
use crate::connections_world::ConnectionsWorld;

#[given(
    regex = r#"^un fichier de connexions qui déclare la première connexion DBeaver "([^"]+)" sous son nom suffixé$"#
)]
async fn file_with_first_suffixed(world: &mut ConnectionsWorld, name: String) {
    let dbeaver = world.context().catalog.clone();
    let discovery = dbeaver.discover();
    let first = discovery
        .connections
        .iter()
        .map(|entry| entry.name.clone())
        .find(|listed| listed.starts_with(&format!("{name} (")))
        .expect("a suffixed DBeaver duplicate");
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{first}\"\nhost = \"db.example.com\"\nuser = \"alice\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"s3cret\"]\n"
        ),
    );
    world.probe = Some(json!(first));
}

#[then(regex = r#"^la connexion "([^"]+)" n'est listée qu'une seule fois$"#)]
async fn connection_listed_once(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(entries(world, &name).len(), 1);
}

fn count_from_source(world: &ConnectionsWorld, source: &str) -> usize {
    connections(world)
        .iter()
        .filter(|connection| connection["source"] == source)
        .count()
}

#[then(regex = r#"^aucune connexion de la source "([^"]+)" n'est listée$"#)]
async fn no_connection_from_source(world: &mut ConnectionsWorld, source: String) {
    assert_eq!(count_from_source(world, &source), 0, "{:?}", world.listing);
}

#[then(regex = r#"^une seule connexion de la source "([^"]+)" est listée$"#)]
async fn one_connection_from_source(world: &mut ConnectionsWorld, source: String) {
    assert_eq!(count_from_source(world, &source), 1, "{:?}", world.listing);
}

#[then(regex = r#"^le doublon déclaré dans le fichier est listé avec la source "([^"]+)"$"#)]
async fn declared_duplicate_source(world: &mut ConnectionsWorld, source: String) {
    let declared = world
        .probe
        .as_ref()
        .and_then(Value::as_str)
        .expect("a declared duplicate")
        .to_string();
    let found = entries(world, &declared);
    assert_eq!(found.len(), 1, "{declared}");
    assert_eq!(found[0]["source"], source);
}

#[then(regex = r"^la liste ne porte aucun indice$")]
async fn no_hint(world: &mut ConnectionsWorld) {
    let listing = world.listing.as_ref().expect("listed");
    assert!(listing.get("hint").is_none(), "{listing}");
}
