use std::sync::Arc;

use russh::client::{self, Handle};
use russh::keys::PublicKeyOrCertificate;

const GATE_VAR: &str = "SCARIF_TEST_MONGODB";
const SSH_PORT_VAR: &str = "SCARIF_TEST_SSH_PORT";
const BASTION_HOST: &str = "localhost";
const BASTION_USER: &str = "tunnel";
const BASTION_PASSWORD: &str = "tunnel";
const MEMBER_HOST: &str = "pl-0.atlas.scarif.test";
const MEMBER_PORTS: [u16; 3] = [1024, 1025, 1026];
const UNLISTED_PORT: u16 = 1027;

struct AcceptAnyKey;

impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

fn enabled() -> bool {
    std::env::var(GATE_VAR).is_ok_and(|value| !value.is_empty())
}

fn bastion_port() -> u16 {
    std::env::var(SSH_PORT_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(55222)
}

async fn bastion() -> Handle<AcceptAnyKey> {
    let config = Arc::new(client::Config::default());
    let mut handle = client::connect(config, (BASTION_HOST, bastion_port()), AcceptAnyKey)
        .await
        .expect("connect to the test bastion");
    let result = handle
        .authenticate_password(BASTION_USER, BASTION_PASSWORD)
        .await
        .expect("authenticate on the test bastion");
    assert!(
        result.success(),
        "the test bastion rejected the tunnel user"
    );
    handle
}

async fn forward(handle: &Handle<AcceptAnyKey>, port: u16) -> Result<(), russh::Error> {
    handle
        .channel_open_direct_tcpip(MEMBER_HOST, u32::from(port), "127.0.0.1", 0)
        .await
        .map(drop)
}

mod integration {
    use super::*;

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn les_trois_membres_du_replica_set_sont_joignables_par_le_bastion() {
        if !enabled() {
            return;
        }
        let handle = bastion().await;
        for port in MEMBER_PORTS {
            forward(&handle, port).await.unwrap_or_else(|error| {
                panic!("{MEMBER_HOST}:{port} through the bastion: {error}")
            });
        }
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn le_bastion_refuse_un_port_mongodb_hors_des_trois_membres() {
        if !enabled() {
            return;
        }
        let handle = bastion().await;
        assert!(forward(&handle, UNLISTED_PORT).await.is_err());
    }
}
