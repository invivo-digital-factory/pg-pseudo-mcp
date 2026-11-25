use cucumber::{then, when};
use scarif_database_mcp::contexts::connections::domain::types::{
    FindRequest, MongoConnectParams, MongoTransport,
};
use serde_json::json;

use crate::connections_steps::support::{LIMITS, assert_single_hop, entry, listed, record};
use crate::connections_world::ConnectionsWorld;

fn mongo(world: &mut ConnectionsWorld, name: &str) -> MongoConnectParams {
    let entry = entry(world, name);
    entry
        .mongo
        .unwrap_or_else(|| panic!("{name} has no MongoDB parameters: {:?}", entry.status))
}

#[then(regex = r#"^la connexion MongoDB "([^"]+)" résout le nom SRV "([^"]+)" avec TLS$"#)]
async fn mongo_srv(world: &mut ConnectionsWorld, name: String, host: String) {
    assert_eq!(mongo(world, &name).transport, MongoTransport::Srv { host });
}

#[then(
    regex = r#"^la connexion MongoDB "([^"]+)" vise le membre "([^"]+)" sur le port (\d+) (avec|sans) TLS$"#
)]
async fn mongo_seed(
    world: &mut ConnectionsWorld,
    name: String,
    host: String,
    port: u16,
    tls: String,
) {
    assert_eq!(
        mongo(world, &name).transport,
        MongoTransport::Seed {
            host,
            port,
            tls: tls == "avec"
        }
    );
}

#[then(regex = r#"^la connexion MongoDB "([^"]+)" déclare le jeu de réplicas "([^"]+)"$"#)]
async fn mongo_replica_set(world: &mut ConnectionsWorld, name: String, replica_set: String) {
    assert_eq!(mongo(world, &name).replica_set, Some(replica_set));
}

#[then(
    regex = r#"^la connexion MongoDB "([^"]+)" passe par le bastion "([^"]+)" en tant que "([^"]+)"$"#
)]
async fn mongo_bastion(world: &mut ConnectionsWorld, name: String, endpoint: String, user: String) {
    let hops = mongo(world, &name).ssh_hops.expect("SSH hops");
    assert_single_hop(&hops, &endpoint, &user);
}

#[then(regex = r#"^la connexion "([^"]+)" est listée sans base par défaut$"#)]
async fn connection_has_no_database(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert!(entry.get("database").is_none(), "{entry}");
}

#[when(regex = r#"^l'agent lit le catalogue de la connexion "([^"]+)"$"#)]
async fn agent_reads_catalog_of(world: &mut ConnectionsWorld, name: String) {
    let context = world.context();
    let outcome = context
        .read_catalog
        .execute_cached(&name, None, "SELECT 1", LIMITS)
        .await
        .map(|_| ());
    record(world, outcome);
}

#[when(
    regex = r#"^l'agent lit les documents de la collection "([^"]+)" sur la connexion "([^"]+)"$"#
)]
async fn agent_finds_on(world: &mut ConnectionsWorld, collection: String, name: String) {
    let context = world.context();
    let request = FindRequest {
        database: Some("shop".to_string()),
        collection,
        filter: json!({}),
        projection: None,
        sort: None,
        skip: None,
        limit: None,
        hint: None,
        collation: None,
    };
    let outcome = context
        .run_find
        .execute(&name, &request, LIMITS)
        .await
        .map(|_| ());
    record(world, outcome);
}
