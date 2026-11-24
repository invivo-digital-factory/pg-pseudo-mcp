use std::sync::Arc;

use russh::client::{self, Handle};
use russh::keys::PublicKeyOrCertificate;

use super::tunnel::{SSH_HOST, SSH_PASSWORD, SSH_USER, ssh_port};

const GATE_VAR: &str = "SCARIF_TEST_MONGODB";
const MEMBER_HOST: &str = "pl-0.atlas.scarif.test";
const MEMBER_PORTS: [u16; 3] = [1024, 1025, 1026];
const UNPERMITTED_HOST: &str = "mongodb";
const UNPERMITTED_PORT: u16 = 1024;

struct AcceptAnyKey;

impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

fn require_gate() {
    assert!(
        std::env::var(GATE_VAR).is_ok_and(|value| !value.is_empty()),
        "{GATE_VAR} must be set to run the tests that need the docker compose mongodb service"
    );
}

async fn bastion() -> Handle<AcceptAnyKey> {
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (SSH_HOST, ssh_port()), AcceptAnyKey)
        .await
        .expect("connect to the test bastion");
    let result = handle
        .authenticate_password(SSH_USER, SSH_PASSWORD)
        .await
        .expect("authenticate on the test bastion");
    assert!(
        result.success(),
        "the test bastion rejected the tunnel user"
    );
    handle
}

async fn forward(handle: &Handle<AcceptAnyKey>, host: &str, port: u16) -> Result<(), russh::Error> {
    handle
        .channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0)
        .await
        .map(drop)
}

mod integration {
    use super::*;

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn les_trois_membres_du_replica_set_sont_joignables_par_le_bastion() {
        require_gate();
        let handle = bastion().await;
        for port in MEMBER_PORTS {
            forward(&handle, MEMBER_HOST, port)
                .await
                .unwrap_or_else(|error| {
                    panic!("{MEMBER_HOST}:{port} through the bastion: {error}")
                });
        }
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn le_bastion_refuse_une_cible_mongodb_qui_ecoute_hors_de_sa_liste() {
        require_gate();
        let handle = bastion().await;
        let refusal = forward(&handle, UNPERMITTED_HOST, UNPERMITTED_PORT)
            .await
            .expect_err("a listening but unpermitted target must be refused");
        assert!(
            matches!(
                refusal,
                russh::Error::ChannelOpenFailure(
                    russh::ChannelOpenFailure::AdministrativelyProhibited
                )
            ),
            "{refusal}"
        );
    }
}
