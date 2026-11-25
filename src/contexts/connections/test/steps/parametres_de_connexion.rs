use cucumber::{then, when};
use scarif_database_mcp::contexts::connections::domain::repositories::ConnectionCatalog;
use scarif_database_mcp::contexts::connections::domain::types::{
    ConnectParams, ConnectionEntry, Discovery, MongoConnectParams, MongoTransport,
};

use crate::connections_world::ConnectionsWorld;

fn discovery(world: &mut ConnectionsWorld) -> Discovery {
    match &world.discovery {
        Some(discovery) => discovery.clone(),
        None => world.context().catalog.discover(),
    }
}

fn entry(world: &mut ConnectionsWorld, name: &str) -> ConnectionEntry {
    discovery(world)
        .connections
        .into_iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("{name} is not discovered"))
}

fn params(world: &mut ConnectionsWorld, name: &str) -> ConnectParams {
    let entry = entry(world, name);
    entry
        .params
        .unwrap_or_else(|| panic!("{name} has no connection parameters: {:?}", entry.status))
}

fn mongo(world: &mut ConnectionsWorld, name: &str) -> MongoConnectParams {
    let entry = entry(world, name);
    entry
        .mongo
        .unwrap_or_else(|| panic!("{name} has no MongoDB parameters: {:?}", entry.status))
}

#[when(regex = r#"^le serveur prépare la connexion "([^"]+)" pour la base "([^"]+)"$"#)]
async fn prepare_for_database(world: &mut ConnectionsWorld, _name: String, database: String) {
    let catalog = world.context().catalog.clone();
    world.discovery = Some(catalog.discover_in(Some(&database)));
}

#[then(regex = r#"^la connexion "([^"]+)" vise le port (\d+) avec le mode TLS "([^"]+)"$"#)]
async fn targets_port_and_tls(world: &mut ConnectionsWorld, name: String, port: u16, tls: String) {
    let params = params(world, &name);
    assert_eq!(params.port, port);
    assert_eq!(params.sslmode.as_str(), tls);
}

#[then(regex = r#"^la connexion "([^"]+)" vise l'hôte "([^"]+)"$"#)]
async fn targets_host(world: &mut ConnectionsWorld, name: String, host: String) {
    assert_eq!(params(world, &name).host, host);
}

#[then(
    regex = r#"^la connexion "([^"]+)" se connecte en tant que "([^"]+)" avec le mot de passe "([^"]+)"$"#
)]
async fn connects_as(world: &mut ConnectionsWorld, name: String, user: String, password: String) {
    let params = params(world, &name);
    assert_eq!(params.user, user);
    assert_eq!(params.password.expose(), password);
}

#[then(regex = r#"^le mot de passe retenu pour la connexion "([^"]+)" est « (.+) »$"#)]
async fn retained_password(world: &mut ConnectionsWorld, name: String, password: String) {
    assert_eq!(params(world, &name).password.expose(), password);
}

#[then(regex = r#"^la connexion "([^"]+)" n'a pas de tunnel SSH$"#)]
async fn has_no_tunnel(world: &mut ConnectionsWorld, name: String) {
    assert!(params(world, &name).tunnel.is_none());
}

#[then(regex = r#"^la connexion "([^"]+)" traverse (\d+) sauts SSH$"#)]
async fn hop_count(world: &mut ConnectionsWorld, name: String, count: usize) {
    let tunnel = params(world, &name).tunnel.expect("a tunnel");
    assert_eq!(tunnel.hops.len(), count);
}

#[then(regex = r#"^la connexion "([^"]+)" traverse les sauts SSH "([^"]+)"$"#)]
async fn hop_list(world: &mut ConnectionsWorld, name: String, hops: String) {
    let tunnel = params(world, &name).tunnel.expect("a tunnel");
    let actual: Vec<String> = tunnel
        .hops
        .iter()
        .map(|hop| format!("{}:{}", hop.host, hop.port))
        .collect();
    assert_eq!(actual.join(", "), hops);
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
