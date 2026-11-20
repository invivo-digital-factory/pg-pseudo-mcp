use cucumber::{given, then, when};
use pg_pseudo_mcp::contexts::connections::domain::types::ExecutionLimits;

use crate::bdd_support::{
    BASTION_HOST, BASTION_USER, CONNECTION, TunnelSpec, bastion_port, database_target,
    known_hosts_pattern, public_key_fields, ssh_asset, write_workspace,
};
use crate::connections_world::ConnectionsWorld;

#[given(regex = r"^une connexion DBeaver qui traverse le bastion SSH de test$")]
async fn tunneled_connection(world: &mut ConnectionsWorld) {
    let mut target = database_target();
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

#[given(regex = r"^un fichier known_hosts qui connaît la clé du bastion$")]
async fn known_hosts_with_bastion_key(world: &mut ConnectionsWorld) {
    let line = format!(
        "{} {}\n",
        known_hosts_pattern(),
        public_key_fields("ssh_host_ed25519_test.pub")
    );
    world.write_known_hosts(&line);
}

#[given(regex = r"^un fichier known_hosts vide$")]
async fn empty_known_hosts(world: &mut ConnectionsWorld) {
    world.write_known_hosts("");
}

#[given(regex = r"^un fichier known_hosts qui porte une autre clé pour le bastion$")]
async fn known_hosts_with_other_key(world: &mut ConnectionsWorld) {
    let line = format!(
        "{} {}\n",
        known_hosts_pattern(),
        public_key_fields("other_host_ed25519_test.pub")
    );
    world.write_known_hosts(&line);
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
            .execute(CONNECTION, "SELECT 1", limits)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string()),
    );
}

#[then(regex = r"^la session s'ouvre et répond$")]
async fn session_answers(world: &mut ConnectionsWorld) {
    assert_eq!(world.outcome, Some(Ok(())));
}

fn refusal(world: &ConnectionsWorld) -> &str {
    match world.outcome.as_ref().expect("a statement was run") {
        Ok(()) => panic!("the connection was accepted"),
        Err(message) => message,
    }
}

#[then(regex = r"^la connexion est refusée$")]
async fn connection_is_refused(world: &mut ConnectionsWorld) {
    refusal(world);
}

#[then(regex = r#"^le message de refus contient "([^"]+)"$"#)]
async fn refusal_contains(world: &mut ConnectionsWorld, expected: String) {
    let message = refusal(world);
    assert!(message.contains(&expected), "{message}");
}

#[then(regex = r#"^le message de refus contient l'empreinte "([^"]+)" de la clé du bastion$"#)]
async fn refusal_carries_fingerprint(world: &mut ConnectionsWorld, prefix: String) {
    let message = refusal(world);
    assert!(message.contains(&prefix), "{message}");
}
