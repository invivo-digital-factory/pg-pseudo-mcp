use base64::Engine;
use cucumber::{given, when};
use hmac::{Hmac, KeyInit, Mac};
use scarif_database_mcp::contexts::connections::domain::types::ExecutionLimits;
use serde_json::json;
use sha1::Sha1;

use crate::bdd_support::{
    BASTION_HOST, BASTION_USER, CONNECTION, TunnelSpec, bastion_port, known_hosts_pattern,
    public_key_fields, ssh_asset, write_workspace,
};
use crate::connections_steps::support::{
    STRICT_NO, bastion_key, database_target_or_offline, dbeaver_dir, test_bastion, write_ssh_config,
};
use crate::connections_world::ConnectionsWorld;

fn tunneled_workspace(world: &mut ConnectionsWorld) {
    let mut target = database_target_or_offline();
    target.host = "postgres".to_string();
    target.port = "5432".to_string();
    let tunnel = TunnelSpec {
        host: BASTION_HOST.to_string(),
        port: bastion_port(),
        user: BASTION_USER.to_string(),
        key_path: ssh_asset("tunnel_test_ed25519"),
    };
    write_workspace(world.workspace.path(), &target, true, Some(&tunnel));
}

#[given(regex = r"^une connexion DBeaver qui traverse le bastion SSH de test$")]
async fn tunneled_connection(world: &mut ConnectionsWorld) {
    tunneled_workspace(world);
}

#[given(
    regex = r"^une connexion DBeaver qui traverse le bastion SSH de test en contournant la vérification de clé d'hôte$"
)]
async fn tunneled_connection_bypassing(world: &mut ConnectionsWorld) {
    tunneled_workspace(world);
    let sources = dbeaver_dir(world, "General").join("data-sources.json");
    let mut document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&sources).expect("data sources"))
            .expect("json");
    document["connections"]["postgres-jdbc-canary"]["configuration"]["handlers"]["ssh_tunnel"]["properties"]
        ["bypassHostVerification"] = json!(true);
    std::fs::write(&sources, document.to_string()).expect("data sources");
}

fn hashed(pattern: &str) -> String {
    let salt = [42u8; 20];
    let mut mac = Hmac::<Sha1>::new_from_slice(&salt).expect("hmac key");
    mac.update(pattern.as_bytes());
    let engine = base64::engine::general_purpose::STANDARD;
    format!(
        "|1|{}|{}",
        engine.encode(salt),
        engine.encode(mac.finalize().into_bytes())
    )
}

#[given(regex = r"^un fichier known_hosts qui connaît la clé du bastion sous une forme hachée$")]
async fn known_hosts_hashed(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!(
        "{} {}\n",
        hashed(&known_hosts_pattern()),
        bastion_key()
    ));
}

#[given(
    regex = r"^un fichier known_hosts qui connaît la clé du bastion après la clé d'un autre hôte$"
)]
async fn known_hosts_after_other(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!(
        "# entries\nother.example.com {}\n{} {}\n",
        public_key_fields("other_host_ed25519_test.pub"),
        known_hosts_pattern(),
        bastion_key()
    ));
}

#[given(regex = r"^un fichier known_hosts vide$")]
async fn empty_known_hosts(world: &mut ConnectionsWorld) {
    world.write_known_hosts("");
}

#[given(regex = r"^un fichier known_hosts qui porte une autre clé pour le bastion$")]
async fn known_hosts_with_other_key(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!(
        "{} {}\n",
        known_hosts_pattern(),
        public_key_fields("other_host_ed25519_test.pub")
    ));
}

#[given(regex = r"^un fichier known_hosts qui connaît la clé du bastion mais la révoque aussi$")]
async fn known_hosts_revoked(world: &mut ConnectionsWorld) {
    let pattern = known_hosts_pattern();
    world.write_known_hosts(&format!(
        "{pattern} {key}\n@revoked {pattern} {key}\n",
        key = bastion_key()
    ));
}

#[given(
    regex = r"^un fichier known_hosts qui ne déclare la clé du bastion que comme autorité de certification$"
)]
async fn known_hosts_cert_authority(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!(
        "@cert-authority {} {}\n",
        known_hosts_pattern(),
        bastion_key()
    ));
}

#[when(regex = r"^l'agent exécute une requête de lecture sur la connexion tunnelée$")]
async fn agent_runs_read_through_tunnel(world: &mut ConnectionsWorld) {
    let context = world.context();
    let limits = ExecutionLimits {
        max_rows: 10,
        timeout_ms: 10_000,
    };
    world.outcome = Some(
        context
            .run_read_only_statement
            .execute(CONNECTION, None, "SELECT 1", limits)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
    );
}

#[given(
    regex = r#"^un fichier de configuration SSH de test avec l'alias "([^"]+)" en StrictHostKeyChecking no$"#
)]
async fn ssh_config_with_alias_strict_no(world: &mut ConnectionsWorld, alias: String) {
    write_ssh_config(world, &test_bastion(&alias, STRICT_NO));
}
