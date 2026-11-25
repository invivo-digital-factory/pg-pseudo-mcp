use cucumber::gherkin::Step;
use cucumber::{given, then};

use crate::connections_steps::support::{docstring, entries, key_of, write_connection_file};
use crate::connections_world::ConnectionsWorld;

#[then(regex = r#"^toutes les connexions nommées "([^"]+)" sont indisponibles$"#)]
async fn all_named_connections_unavailable(world: &mut ConnectionsWorld, name: String) {
    let found = entries(world, &name);
    assert_eq!(found.len(), 2, "both duplicates are listed");
    assert!(found.iter().all(|entry| entry["status"] == "unavailable"));
}

#[given(
    regex = r#"^le fichier de connexions qui déclare l'entrée suivante sans son champ "([^"]+)" :$"#
)]
async fn entry_without_field(world: &mut ConnectionsWorld, field: String, step: &Step) {
    let entry = docstring(step);
    assert!(
        entry.lines().any(|line| key_of(line) == field),
        "the entry must declare {field} before the scenario drops it"
    );
    let kept: Vec<&str> = entry.lines().filter(|line| key_of(line) != field).collect();
    write_connection_file(world, &kept.join("\n"));
}
