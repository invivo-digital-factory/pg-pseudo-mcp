use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fast_socks5::client::{Config, Socks5Stream};
use fast_socks5::{ReplyError, SocksError};
use russh::client::{self, Handle};
use russh::keys::PublicKeyOrCertificate;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::tunnel::{SSH_HOST, SSH_PASSWORD, SSH_USER, listing_container_key, ssh_port};
use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::{Hop, Tunnel, TunnelAuth};
use crate::contexts::connections::infrastructure::data_sources::socks::{
    AllowedTarget, ChannelOpener, SocksFront, TargetPolicy,
};
use crate::contexts::connections::infrastructure::data_sources::ssh::{SshLink, TunnelStream};

const GATE_VAR: &str = "SCARIF_TEST_MONGODB";
pub(super) const MEMBER_HOST: &str = "pl-0.atlas.scarif.test";
const MEMBER_PORTS: [u16; 3] = [1024, 1025, 1026];
const UNPERMITTED_HOST: &str = "mongodb";
const UNPERMITTED_PORT: u16 = 1024;
const OP_MSG: i32 = 2013;
const HEADER_LEN: usize = 16;

struct AcceptAnyKey;

impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _: &PublicKeyOrCertificate) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub(super) fn require_gate() {
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

struct CountingLink {
    link: SshLink,
    opened: AtomicUsize,
}

impl ChannelOpener for CountingLink {
    type Stream = TunnelStream;

    async fn open_channel(&self, target: &AllowedTarget) -> Result<TunnelStream, SessionError> {
        self.opened.fetch_add(1, Ordering::SeqCst);
        self.link.open_channel(target).await
    }
}

async fn counting_link(known_hosts: &Path) -> Arc<CountingLink> {
    let tunnel = Tunnel {
        hops: vec![Hop {
            label: SSH_HOST.to_string(),
            host: SSH_HOST.to_string(),
            port: ssh_port(),
            user: SSH_USER.to_string(),
            auth: TunnelAuth::Password(SSH_PASSWORD.to_string()),
            verify_host_key: true,
            known_hosts: vec![known_hosts.to_path_buf()],
            host_key_alias: None,
        }],
        remote_host: MEMBER_HOST.to_string(),
        remote_port: MEMBER_PORTS[0],
    };
    let link = SshLink::open(&tunnel)
        .await
        .expect("open the SSH link with a verified host key");
    Arc::new(CountingLink {
        link,
        opened: AtomicUsize::new(0),
    })
}

fn hello(request_id: i32) -> Vec<u8> {
    let mut document = vec![0x10];
    document.extend_from_slice(b"hello\0");
    document.extend_from_slice(&1i32.to_le_bytes());
    document.push(0x02);
    document.extend_from_slice(b"$db\0");
    document.extend_from_slice(&6i32.to_le_bytes());
    document.extend_from_slice(b"admin\0");
    document.push(0);
    let document_len = i32::try_from(document.len() + 4).expect("small document");
    let mut body = 0u32.to_le_bytes().to_vec();
    body.push(0);
    body.extend_from_slice(&document_len.to_le_bytes());
    body.extend_from_slice(&document);
    let total = i32::try_from(HEADER_LEN + body.len()).expect("small message");
    let mut message = Vec::new();
    for field in [total, request_id, 0, OP_MSG] {
        message.extend_from_slice(&field.to_le_bytes());
    }
    message.extend_from_slice(&body);
    message
}

fn field(header: &[u8; HEADER_LEN], index: usize) -> i32 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&header[index * 4..index * 4 + 4]);
    i32::from_le_bytes(bytes)
}

async fn hello_through(front: &SocksFront, port: u16) -> Vec<u8> {
    let credentials = front.credentials();
    let mut stream = Socks5Stream::connect_with_password(
        front.address(),
        MEMBER_HOST.to_string(),
        port,
        credentials.username().to_string(),
        credentials.password().to_string(),
        Config::default(),
    )
    .await
    .unwrap_or_else(|error| panic!("CONNECT {MEMBER_HOST}:{port}: {error}"));
    let request_id = i32::from(port);
    stream
        .write_all(&hello(request_id))
        .await
        .expect("send hello");
    let mut header = [0u8; HEADER_LEN];
    stream.read_exact(&mut header).await.expect("hello header");
    assert_eq!(field(&header, 2), request_id, "reply to our hello");
    assert_eq!(field(&header, 3), OP_MSG);
    let length = usize::try_from(field(&header, 0)).expect("positive length");
    let mut body = vec![0u8; length - HEADER_LEN];
    stream.read_exact(&mut body).await.expect("hello body");
    body
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn me_element(address: &str) -> Vec<u8> {
    let mut element = vec![0x02];
    element.extend_from_slice(b"me\0");
    let length = i32::try_from(address.len() + 1).expect("short address");
    element.extend_from_slice(&length.to_le_bytes());
    element.extend_from_slice(address.as_bytes());
    element.push(0);
    element
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

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn deux_connexions_concurrentes_vers_deux_membres_partagent_un_seul_lien_ssh() {
        require_gate();
        let known_hosts = listing_container_key();
        let link = counting_link(&known_hosts.path).await;
        let policy = TargetPolicy::from_seed_host(MEMBER_HOST).expect("valid seed host");
        let front = SocksFront::start(link.clone(), policy)
            .await
            .expect("start the SOCKS5 front");
        let (first, second) = tokio::join!(
            hello_through(&front, MEMBER_PORTS[0]),
            hello_through(&front, MEMBER_PORTS[1])
        );
        for (reply, port) in [(first, MEMBER_PORTS[0]), (second, MEMBER_PORTS[1])] {
            assert!(contains(&reply, b"rs0"), "{port}: the member names its set");
            assert!(
                contains(&reply, &me_element(&format!("{MEMBER_HOST}:{port}"))),
                "{port}: the reply comes from that member"
            );
        }
        assert_eq!(link.opened.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and mongodb services"]
    async fn le_front_refuse_une_cible_que_le_bastion_aurait_ouverte() {
        require_gate();
        let known_hosts = listing_container_key();
        let link = counting_link(&known_hosts.path).await;
        let policy = TargetPolicy::from_seed_host(MEMBER_HOST).expect("valid seed host");
        let front = SocksFront::start(link.clone(), policy)
            .await
            .expect("start the SOCKS5 front");
        let credentials = front.credentials();
        let refused = Socks5Stream::connect_with_password(
            front.address(),
            SSH_HOST.to_string(),
            22,
            credentials.username().to_string(),
            credentials.password().to_string(),
            Config::default(),
        )
        .await;
        assert!(
            matches!(
                refused,
                Err(SocksError::ReplyError(ReplyError::ConnectionNotAllowed))
            ),
            "the front answers connection not allowed"
        );
        assert_eq!(link.opened.load(Ordering::SeqCst), 0);
    }
}
