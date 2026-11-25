use cucumber::given;
use serde_json::{Value, json};

use crate::bdd_support::offline_target;
use crate::connections_steps::support::write_dbeaver;
use crate::connections_world::ConnectionsWorld;

#[given(
    regex = r#"^un espace de travail DBeaver avec les connexions "([^"]+)" et "([^"]+)" dont les mots de passe sont enregistrés$"#
)]
async fn workspace_with_two_connections(
    world: &mut ConnectionsWorld,
    first: String,
    second: String,
) {
    let target = offline_target();
    let connections: Vec<(String, Value, Value)> = [first, second]
        .into_iter()
        .map(|name| {
            (
                format!("postgres-jdbc-{name}"),
                json!({
                    "provider": "postgresql",
                    "driver": "postgres-jdbc",
                    "name": name,
                    "save-password": true,
                    "configuration": {
                        "host": target.host,
                        "port": target.port,
                        "database": target.database,
                        "type": "dev",
                        "auth-model": "native",
                        "provider-properties": { "sslmode": "disable" }
                    }
                }),
                json!({ "#connection": { "user": target.user, "password": target.password } }),
            )
        })
        .collect();
    write_dbeaver(world, "General", "", &connections);
}
