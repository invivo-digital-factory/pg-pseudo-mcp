use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::domain::errors::SessionError;
use scarif_database_mcp::contexts::connections::domain::types::{
    ConnectOptions, Hop, HostKeyPolicy, MongoConnectParams, MongoTransport, SshAuth, SshHop,
    SslMode, Tunnel, TunnelAuth,
};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::socks::{
    ProxyCredentials, TargetPolicy,
};
use scarif_database_mcp::contexts::connections::infrastructure::mappers::{
    MongoRedaction, redact_connection_identity,
};
use serde_json::{Value, json};

use crate::connections_world::ConnectionsWorld;

fn identity(world: &ConnectionsWorld) -> &Value {
    world.probe.as_ref().expect("a connection identity")
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().expect("a text field").to_string()
}

#[given(
    regex = r#"^une connexion PostgreSQL vers l'hôte "([^"]+)" en tant que "([^"]+)", par les sauts SSH "([^"]+)" en tant que "([^"]+)" puis "([^"]+)" en tant que "([^"]+)"$"#
)]
async fn postgres_identity(
    world: &mut ConnectionsWorld,
    host: String,
    user: String,
    first_hop: String,
    first_user: String,
    second_hop: String,
    second_user: String,
) {
    world.probe = Some(json!({
        "host": host, "user": user,
        "hops": [[first_hop, first_user], [second_hop, second_user]],
    }));
}

fn postgres_options(identity: &Value) -> ConnectOptions {
    let host = text(identity, "host");
    let hops = identity["hops"]
        .as_array()
        .expect("hops")
        .iter()
        .map(|hop| Hop {
            label: "bastion".to_string(),
            host: hop[0].as_str().expect("hop host").to_string(),
            port: 22,
            user: hop[1].as_str().expect("hop user").to_string(),
            auth: TunnelAuth::Agent,
            verify_host_key: true,
            known_hosts: Vec::new(),
            host_key_alias: None,
        })
        .collect();
    ConnectOptions {
        host: host.clone(),
        port: 5432,
        database: "shop".to_string(),
        user: text(identity, "user"),
        password: "s3cret".to_string(),
        sslmode: SslMode::Require,
        tunnel: Some(Tunnel {
            hops,
            remote_host: host,
            remote_port: 5432,
        }),
    }
}

#[when(regex = r"^la connexion échoue en (connexion|authentification) avec le message « (.+) »$")]
async fn postgres_failure(world: &mut ConnectionsWorld, nature: String, message: String) {
    let options = postgres_options(identity(world));
    let error = if nature == "connexion" {
        SessionError::Connect(message)
    } else {
        SessionError::Authentication(message)
    };
    let rendered = match redact_connection_identity(error, &options) {
        SessionError::Connect(text) | SessionError::Authentication(text) => text,
        other => panic!("the nature of the error changed: {other:?}"),
    };
    world.outcome = Some(Err(rendered));
}

#[given(
    regex = r#"^une connexion MongoDB vers le nom SRV "([^"]+)" en tant que "([^"]+)", par le saut SSH "([^"]+)" en tant que "([^"]+)", avec un front SOCKS5$"#
)]
async fn mongo_identity(
    world: &mut ConnectionsWorld,
    host: String,
    user: String,
    hop: String,
    hop_user: String,
) {
    let proxy = ProxyCredentials::generate().expect("proxy credentials");
    world.probe = Some(json!({
        "host": host, "user": user, "hop": hop, "hop_user": hop_user,
        "proxy": [proxy.username(), proxy.password()],
    }));
    world.proxy = Some(proxy);
}

#[when(regex = r"^le pilote MongoDB échoue avec le message « (.+) »$")]
async fn mongo_failure(world: &mut ConnectionsWorld, message: String) {
    let identity = identity(world).clone();
    let host = text(&identity, "host");
    let params = MongoConnectParams {
        transport: MongoTransport::Srv { host: host.clone() },
        replica_set: None,
        database: None,
        user: text(&identity, "user"),
        password_command: vec!["/bin/echo".to_string()],
        ssh_hops: Some(vec![SshHop {
            label: "bastion".to_string(),
            host: text(&identity, "hop"),
            port: 22,
            user: text(&identity, "hop_user"),
            auth: SshAuth::Agent,
            host_key: HostKeyPolicy::Verify {
                known_hosts: Vec::new(),
                alias: None,
            },
        }]),
    };
    let proxy = world.proxy.as_ref().expect("proxy credentials");
    let message = message.replace(
        "{identifiants du front}",
        &format!("{}:{}", proxy.username(), proxy.password()),
    );
    let members = TargetPolicy::from_srv_name(&host).expect("valid SRV name");
    let redaction = MongoRedaction::new(&params, Some(members), Some(proxy));
    world.outcome = Some(Err(redaction.redact(&message)));
}

#[then(regex = r"^l'erreur rendue est « (.+) »$")]
async fn rendered_error(world: &mut ConnectionsWorld, expected: String) {
    assert_eq!(world.outcome, Some(Err(expected)));
}
