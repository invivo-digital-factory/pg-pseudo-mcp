use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use russh::client;
use russh::keys::PublicKeyOrCertificate;
use sha1::Sha1;
use tokio::net::TcpListener;
use tokio_postgres::Config;

use super::database::database_url;
use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::{
    Cell, ConnectOptions, SslMode, Tunnel, TunnelAuth,
};
use crate::contexts::connections::infrastructure::data_sources::postgres::{
    Session, connect, connect_within,
};

const SSH_PORT_VAR: &str = "PG_PSEUDO_TEST_SSH_PORT";
const SSH_HOST: &str = "localhost";
const SSH_USER: &str = "tunnel";
const SSH_PASSWORD: &str = "tunnel";
const DROPPED_USER: &str = "dropped";
const KEY_PASSPHRASE: &str = "pass";
const REMOTE_HOST: &str = "postgres";
const REMOTE_PORT: u16 = 5432;
const TIMEOUT_MS: u64 = 10_000;

fn ssh_port() -> u16 {
    std::env::var(SSH_PORT_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(55222)
}

fn asset(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ssh")
        .join(name)
}

fn public_key_fields(name: &str) -> String {
    let line = std::fs::read_to_string(asset(name)).expect("public key file");
    let mut fields = line.split_whitespace();
    let algorithm = fields.next().expect("key algorithm");
    let key = fields.next().expect("key data");
    format!("{algorithm} {key}")
}

fn host_pattern() -> String {
    match ssh_port() {
        22 => SSH_HOST.to_string(),
        port => format!("[{SSH_HOST}]:{port}"),
    }
}

fn hashed_host_pattern() -> String {
    let salt = [42u8; 20];
    let mut mac = Hmac::<Sha1>::new_from_slice(&salt).expect("hmac key");
    mac.update(host_pattern().as_bytes());
    let engine = base64::engine::general_purpose::STANDARD;
    format!(
        "|1|{}|{}",
        engine.encode(salt),
        engine.encode(mac.finalize().into_bytes())
    )
}

struct KnownHosts {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn known_hosts(contents: &str) -> KnownHosts {
    let dir = tempfile::tempdir().expect("known_hosts dir");
    let path = dir.path().join("known_hosts");
    std::fs::write(&path, contents).expect("known_hosts file");
    KnownHosts { _dir: dir, path }
}

fn listing_container_key() -> KnownHosts {
    known_hosts(&format!(
        "# test entries\nother.example ssh-ed25519 {}\n{} {}\n",
        public_key_fields("other_host_ed25519_test.pub")
            .split_whitespace()
            .nth(1)
            .expect("key data"),
        host_pattern(),
        public_key_fields("ssh_host_ed25519_test.pub"),
    ))
}

fn options(auth: TunnelAuth, verify_host_key: bool, known_hosts: Option<&Path>) -> ConnectOptions {
    let config = Config::from_str(&database_url()).expect("valid database url");
    ConnectOptions {
        host: REMOTE_HOST.to_string(),
        port: REMOTE_PORT,
        database: config.get_dbname().unwrap_or("pgpseudo").to_string(),
        user: config.get_user().unwrap_or("pgpseudo").to_string(),
        password: config
            .get_password()
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .unwrap_or_default(),
        sslmode: SslMode::Disable,
        tunnel: Some(Tunnel {
            host: SSH_HOST.to_string(),
            port: ssh_port(),
            user: SSH_USER.to_string(),
            auth,
            remote_host: REMOTE_HOST.to_string(),
            remote_port: REMOTE_PORT,
            verify_host_key,
            known_hosts: known_hosts.map(Path::to_path_buf),
        }),
    }
}

fn key_auth() -> TunnelAuth {
    TunnelAuth::PublicKey {
        key_path: asset("tunnel_test_ed25519"),
        passphrase: None,
    }
}

fn unverified(auth: TunnelAuth) -> ConnectOptions {
    options(auth, false, None)
}

async fn backend_pid(session: &mut Session) -> Cell {
    let output = session
        .run_read_only("SELECT pg_backend_pid()", 10, TIMEOUT_MS)
        .await
        .expect("backend pid through the tunnel");
    output.rows.into_iter().next().expect("one row").remove(0)
}

async fn select_one(session: &mut Session) {
    let output = session
        .run_read_only("SELECT 1 AS one", 10, TIMEOUT_MS)
        .await
        .expect("SELECT 1 through the tunnel");
    assert_eq!(output.rows.len(), 1);
}

async fn connect_error(options: &ConnectOptions) -> String {
    match connect(options).await {
        Ok(_) => panic!("the connection must be refused"),
        Err(SessionError::Connect(message)) => message,
        Err(other) => panic!("expected a connection error, got {other:?}"),
    }
}

mod integration {
    use super::*;

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_cle_ssh_ouvre_un_tunnel_vers_la_base() {
        let mut session = connect(&unverified(key_auth()))
            .await
            .expect("connect through the tunnel");
        select_one(&mut session).await;
        let output = session
            .run_read_only("SELECT email FROM customers", 100, TIMEOUT_MS)
            .await
            .expect("select customers");
        assert!(!output.rows.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn le_mode_tls_prefere_est_negocie_a_travers_le_tunnel() {
        let mut options = unverified(key_auth());
        options.sslmode = SslMode::Prefer;
        let mut session = connect(&options)
            .await
            .expect("connect with sslmode prefer through the tunnel");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_cle_chiffree_s_ouvre_avec_sa_phrase_secrete() {
        let mut session = connect(&unverified(TunnelAuth::PublicKey {
            key_path: asset("tunnel_test_ed25519_encrypted"),
            passphrase: Some(KEY_PASSPHRASE.to_string()),
        }))
        .await
        .expect("connect with the decrypted key");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_cle_chiffree_sans_phrase_secrete_est_refusee_clairement() {
        let message = connect_error(&unverified(TunnelAuth::PublicKey {
            key_path: asset("tunnel_test_ed25519_encrypted"),
            passphrase: None,
        }))
        .await;
        assert!(message.contains("encrypted"), "{message}");
        assert!(message.contains("passphrase"), "{message}");
        assert!(
            message.contains("tunnel_test_ed25519_encrypted"),
            "{message}"
        );
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_phrase_secrete_erronee_est_refusee_sans_etre_affichee() {
        let message = connect_error(&unverified(TunnelAuth::PublicKey {
            key_path: asset("tunnel_test_ed25519_encrypted"),
            passphrase: Some("not-the-passphrase-7731".to_string()),
        }))
        .await;
        assert!(message.contains("passphrase"), "{message}");
        assert!(!message.contains("not-the-passphrase-7731"), "{message}");
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn un_mot_de_passe_ssh_ouvre_un_tunnel_vers_la_base() {
        let mut session = connect(&unverified(TunnelAuth::Password(SSH_PASSWORD.to_string())))
            .await
            .expect("connect with the ssh password");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn un_mot_de_passe_ssh_rejete_est_signale_sans_etre_affiche() {
        let message = connect_error(&unverified(TunnelAuth::Password(
            "wrong-ssh-secret-5521".to_string(),
        )))
        .await;
        assert!(message.contains("rejected"), "{message}");
        assert!(message.contains(SSH_USER), "{message}");
        assert!(!message.contains("wrong-ssh-secret-5521"), "{message}");
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_cle_d_hote_listee_dans_known_hosts_est_acceptee() {
        let file = listing_container_key();
        let mut session = connect(&options(key_auth(), true, Some(&file.path)))
            .await
            .expect("connect with a verified host key");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn le_type_de_cle_d_hote_deja_connu_est_negocie_en_premier() {
        let file = known_hosts(&format!(
            "{} {}\n",
            host_pattern(),
            public_key_fields("ssh_host_ecdsa_test.pub")
        ));
        let mut session = connect(&options(key_auth(), true, Some(&file.path)))
            .await
            .expect("connect with only the ecdsa host key recorded");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_entree_known_hosts_hachee_est_acceptee() {
        let file = known_hosts(&format!(
            "{} {}\n",
            hashed_host_pattern(),
            public_key_fields("ssh_host_ed25519_test.pub")
        ));
        let mut session = connect(&options(key_auth(), true, Some(&file.path)))
            .await
            .expect("connect with a hashed known_hosts entry");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_cle_d_hote_inconnue_est_refusee_avec_des_instructions() {
        let file = known_hosts("");
        let message = connect_error(&options(key_auth(), true, Some(&file.path))).await;
        assert!(message.contains("not in known_hosts"), "{message}");
        assert!(
            message.contains(&format!("ssh-keyscan -p {} {SSH_HOST}", ssh_port())),
            "{message}"
        );
        assert!(message.contains("SHA256:"), "{message}");
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_cle_d_hote_modifiee_est_refusee() {
        let file = known_hosts(&format!(
            "{} {}\n",
            host_pattern(),
            public_key_fields("other_host_ed25519_test.pub")
        ));
        let message = connect_error(&options(key_auth(), true, Some(&file.path))).await;
        assert!(message.contains("changed"), "{message}");
        assert!(!message.contains("not in known_hosts"), "{message}");
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn la_verification_d_hote_peut_etre_desactivee_sans_fichier_known_hosts() {
        let missing = tempfile::tempdir().expect("dir");
        let options = options(key_auth(), false, Some(&missing.path().join("absent")));
        let mut session = connect(&options)
            .await
            .expect("connect without verification");
        select_one(&mut session).await;
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_redirection_refusee_nomme_la_destination_distante() {
        let mut options = unverified(key_auth());
        if let Some(tunnel) = options.tunnel.as_mut() {
            tunnel.remote_port = 1;
        }
        let message = connect_error(&options).await;
        assert!(message.contains("postgres:1"), "{message}");
        assert!(message.contains("forward"), "{message}");
    }

    #[tokio::test]
    #[ignore = "needs the docker compose ssh and postgres services"]
    async fn une_session_ssh_coupee_est_retablie_a_la_requete_suivante() {
        let mut options = unverified(key_auth());
        if let Some(tunnel) = options.tunnel.as_mut() {
            tunnel.user = DROPPED_USER.to_string();
        }
        let mut session = connect(&options).await.expect("connect through the tunnel");
        let before = backend_pid(&mut session).await;
        kill_ssh_sessions_of(DROPPED_USER).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let after = backend_pid(&mut session).await;
        assert_ne!(
            before, after,
            "the query must run on a new tunneled connection"
        );
    }

    #[tokio::test]
    async fn un_serveur_ssh_fige_expire_dans_le_delai_de_connexion() {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("address").port();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        let options = ConnectOptions {
            host: REMOTE_HOST.to_string(),
            port: REMOTE_PORT,
            database: "app".to_string(),
            user: "reader".to_string(),
            password: "p".to_string(),
            sslmode: SslMode::Disable,
            tunnel: Some(Tunnel {
                host: "127.0.0.1".to_string(),
                port,
                user: SSH_USER.to_string(),
                auth: TunnelAuth::Password("ssh-secret-0042".to_string()),
                remote_host: REMOTE_HOST.to_string(),
                remote_port: REMOTE_PORT,
                verify_host_key: false,
                known_hosts: None,
            }),
        };
        let started = Instant::now();
        let error = match connect_within(&options, Duration::from_millis(400)).await {
            Ok(_) => panic!("a stalled ssh server cannot open a tunnel"),
            Err(error) => error.to_string(),
        };
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(error.contains("SSH"), "{error}");
        assert!(error.contains("timed out"), "{error}");
        assert!(!error.contains("ssh-secret-0042"), "{error}");
    }

    struct AnyHostKey;

    impl client::Handler for AnyHostKey {
        type Error = russh::Error;

        async fn check_server_key(
            &mut self,
            _: &PublicKeyOrCertificate,
        ) -> Result<bool, Self::Error> {
            Ok(true)
        }
    }

    async fn kill_ssh_sessions_of(user: &str) {
        let config = Arc::new(client::Config::default());
        let mut handle = client::connect(config, (SSH_HOST, ssh_port()), AnyHostKey)
            .await
            .expect("admin ssh connection");
        let auth = handle
            .authenticate_password(user, user)
            .await
            .expect("admin ssh authentication");
        assert!(auth.success());
        let mut channel = handle
            .channel_open_session()
            .await
            .expect("admin ssh session");
        channel
            .exec(false, "kill -KILL -1")
            .await
            .expect("exec kill");
        let _ = tokio::time::timeout(Duration::from_secs(5), async {
            while channel.wait().await.is_some() {}
        })
        .await;
    }
}
