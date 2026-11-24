use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fast_socks5::client::{Config, Socks5Stream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::TcpStream;

use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::infrastructure::data_sources::socks::{
    AllowedTarget, ChannelOpener, SocksFront, TargetPolicy,
};

const MEMBER: &str = "pl-0.atlas.scarif.test";
const SOCKS_VERSION: u8 = 0x05;
const NO_AUTH: u8 = 0x00;
const PASSWORD_AUTH: u8 = 0x02;
const NO_ACCEPTABLE_METHOD: u8 = 0xff;
const CONNECT: u8 = 0x01;
const BIND: u8 = 0x02;
const UDP_ASSOCIATE: u8 = 0x03;
const DOMAIN: u8 = 0x03;
const IPV4: u8 = 0x01;
const IPV6: u8 = 0x04;
const SUCCEEDED: u8 = 0x00;
const NOT_ALLOWED: u8 = 0x02;
const GENERAL_FAILURE: u8 = 0x01;
const COMMAND_NOT_SUPPORTED: u8 = 0x07;
const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(20);
const WAIT: Duration = Duration::from_secs(5);
const PENDING_NEGOTIATIONS: usize = 32;

#[derive(Default)]
struct CountingOpener {
    opened: Mutex<Vec<(String, u16)>>,
    refuse: bool,
}

impl CountingOpener {
    fn refusing() -> Self {
        Self {
            refuse: true,
            ..Self::default()
        }
    }

    fn opened(&self) -> Vec<(String, u16)> {
        self.opened.lock().expect("opened lock").clone()
    }
}

impl ChannelOpener for CountingOpener {
    type Stream = DuplexStream;

    async fn open_channel(&self, target: &AllowedTarget) -> Result<DuplexStream, SessionError> {
        self.opened
            .lock()
            .expect("opened lock")
            .push((target.host().to_string(), target.port()));
        if self.refuse {
            return Err(SessionError::Connect("refused by the fake bastion".into()));
        }
        let (near, far) = tokio::io::duplex(1024);
        tokio::spawn(async move {
            let (mut reader, mut writer) = tokio::io::split(far);
            let _ = tokio::io::copy(&mut reader, &mut writer).await;
        });
        Ok(near)
    }
}

async fn front_with(opener: Arc<CountingOpener>) -> SocksFront {
    let policy = TargetPolicy::from_seed_host(MEMBER).expect("valid seed host");
    SocksFront::start(opener, policy)
        .await
        .expect("start the SOCKS5 front")
}

async fn read_array<const N: usize>(stream: &mut TcpStream) -> [u8; N] {
    let mut buffer = [0u8; N];
    tokio::time::timeout(WAIT, stream.read_exact(&mut buffer))
        .await
        .expect("the front answers in time")
        .expect("read the front answer");
    buffer
}

async fn authenticated(front: &SocksFront, username: &str, password: &str) -> (TcpStream, u8) {
    let mut stream = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    stream
        .write_all(&[SOCKS_VERSION, 1, PASSWORD_AUTH])
        .await
        .expect("send methods");
    assert_eq!(
        read_array::<2>(&mut stream).await,
        [SOCKS_VERSION, PASSWORD_AUTH]
    );
    let mut request = vec![1, u8::try_from(username.len()).expect("short username")];
    request.extend_from_slice(username.as_bytes());
    request.push(u8::try_from(password.len()).expect("short password"));
    request.extend_from_slice(password.as_bytes());
    stream.write_all(&request).await.expect("send credentials");
    let [_, status] = read_array::<2>(&mut stream).await;
    (stream, status)
}

async fn connect_request(front: &SocksFront, address: &[u8], port: u16) -> (TcpStream, u8) {
    command_request(front, CONNECT, address, port).await
}

async fn command_request(
    front: &SocksFront,
    command: u8,
    address: &[u8],
    port: u16,
) -> (TcpStream, u8) {
    let credentials = front.credentials();
    let (mut stream, status) =
        authenticated(front, credentials.username(), credentials.password()).await;
    assert_eq!(status, SUCCEEDED, "the session credentials are accepted");
    let mut request = vec![SOCKS_VERSION, command, 0];
    request.extend_from_slice(address);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await.expect("send the request");
    let reply = read_array::<10>(&mut stream).await;
    assert_eq!(reply[0], SOCKS_VERSION);
    (stream, reply[1])
}

fn domain(name: &str) -> Vec<u8> {
    let mut address = vec![DOMAIN, u8::try_from(name.len()).expect("short name")];
    address.extend_from_slice(name.as_bytes());
    address
}

async fn closed(stream: &mut TcpStream) -> bool {
    let mut buffer = [0u8; 1];
    matches!(
        tokio::time::timeout(WAIT, stream.read(&mut buffer)).await,
        Ok(Ok(0) | Err(_))
    )
}

#[tokio::test]
async fn une_connexion_vers_un_membre_admis_est_relayee_par_un_canal() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let credentials = front.credentials();
    let mut client = Socks5Stream::connect_with_password(
        front.address(),
        MEMBER.to_string(),
        1025,
        credentials.username().to_string(),
        credentials.password().to_string(),
        Config::default(),
    )
    .await
    .expect("CONNECT through the front");
    client.write_all(b"hello").await.expect("write through");
    let mut echoed = [0u8; 5];
    tokio::time::timeout(WAIT, client.read_exact(&mut echoed))
        .await
        .expect("echo in time")
        .expect("read the echo");
    assert_eq!(&echoed, b"hello");
    assert_eq!(opener.opened(), vec![(MEMBER.to_string(), 1025)]);
}

#[tokio::test]
async fn le_nom_transmis_au_bastion_est_le_nom_verifie_en_minuscules() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let (_stream, reply) = connect_request(&front, &domain("PL-1.Atlas.Scarif.Test"), 1026).await;
    assert_eq!(reply, SUCCEEDED);
    assert_eq!(
        opener.opened(),
        vec![("pl-1.atlas.scarif.test".to_string(), 1026)]
    );
}

#[tokio::test]
async fn une_connexion_vers_un_domaine_etranger_est_refusee_sans_ouvrir_de_canal() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    for name in ["evil.example.com", "evilatlas.scarif.test", "127.0.0.1"] {
        let (mut stream, reply) = connect_request(&front, &domain(name), 1024).await;
        assert_eq!(reply, NOT_ALLOWED, "{name}");
        assert!(closed(&mut stream).await, "{name}: connection closed");
    }
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn une_connexion_vers_une_adresse_ip_est_refusee_sans_ouvrir_de_canal() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let (mut stream, reply) = connect_request(&front, &[IPV4, 127, 0, 0, 1], 22).await;
    assert_eq!(reply, NOT_ALLOWED);
    assert!(closed(&mut stream).await);
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn un_canal_refuse_par_le_bastion_rend_un_echec_general() {
    let opener = Arc::new(CountingOpener::refusing());
    let front = front_with(opener.clone()).await;
    let (mut stream, reply) = connect_request(&front, &domain(MEMBER), 1024).await;
    assert_eq!(reply, GENERAL_FAILURE);
    assert!(closed(&mut stream).await);
    assert_eq!(opener.opened().len(), 1);
}

#[tokio::test]
async fn un_client_sans_methode_par_mot_de_passe_est_refuse_a_la_poignee_de_main() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let mut stream = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    stream
        .write_all(&[SOCKS_VERSION, 1, NO_AUTH])
        .await
        .expect("send methods");
    assert_eq!(
        read_array::<2>(&mut stream).await,
        [SOCKS_VERSION, NO_ACCEPTABLE_METHOD]
    );
    assert!(closed(&mut stream).await);
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn un_client_sans_les_identifiants_de_session_est_refuse_a_la_poignee_de_main() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let other = front_with(Arc::new(CountingOpener::default())).await;
    let username = front.credentials().username().to_string();
    let attempts = [
        (username.clone(), "wrong".to_string()),
        (
            other.credentials().username().to_string(),
            other.credentials().password().to_string(),
        ),
        (username, other.credentials().password().to_string()),
        (
            other.credentials().username().to_string(),
            front.credentials().password().to_string(),
        ),
    ];
    for (username, password) in attempts {
        let (mut stream, status) = authenticated(&front, &username, &password).await;
        assert_ne!(status, SUCCEEDED);
        assert!(closed(&mut stream).await);
    }
    let refused = Socks5Stream::connect_with_password(
        front.address(),
        MEMBER.to_string(),
        1024,
        "scarif".to_string(),
        "scarif".to_string(),
        Config::default(),
    )
    .await;
    assert!(refused.is_err());
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn le_front_n_ecoute_que_sur_la_boucle_locale() {
    let front = front_with(Arc::new(CountingOpener::default())).await;
    assert_eq!(front.address().ip(), Ipv4Addr::LOCALHOST);
    assert_ne!(front.address().port(), 0);
}

#[tokio::test]
async fn chaque_session_a_ses_propres_identifiants_aleatoires() {
    let first = front_with(Arc::new(CountingOpener::default())).await;
    let second = front_with(Arc::new(CountingOpener::default())).await;
    for credentials in [first.credentials(), second.credentials()] {
        assert_eq!(credentials.username().len(), 64);
        assert_eq!(credentials.password().len(), 64);
        assert_ne!(credentials.username(), credentials.password());
    }
    assert_ne!(
        first.credentials().password(),
        second.credentials().password()
    );
    assert_ne!(
        first.credentials().username(),
        second.credentials().username()
    );
    assert!(!format!("{:?}", first.credentials()).contains(first.credentials().password()));
}

#[tokio::test]
async fn le_front_et_ses_relais_se_ferment_avec_la_session() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let address = front.address();
    let (mut relayed, reply) = connect_request(&front, &domain(MEMBER), 1024).await;
    assert_eq!(reply, SUCCEEDED);
    drop(front);
    assert!(closed(&mut relayed).await, "the relayed connection ends");
    let mut refused = false;
    for _ in 0..50 {
        if TcpStream::connect(address).await.is_err() {
            refused = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(refused, "the listener is closed");
}

#[tokio::test]
async fn les_commandes_bind_et_udp_associate_sont_refusees_sans_ouvrir_de_canal() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    for command in [BIND, UDP_ASSOCIATE] {
        let (mut stream, reply) = command_request(&front, command, &domain(MEMBER), 1024).await;
        assert_eq!(reply, COMMAND_NOT_SUPPORTED, "command {command}");
        assert!(
            closed(&mut stream).await,
            "command {command}: connection closed"
        );
    }
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn une_adresse_ipv6_est_refusee_sans_ouvrir_de_canal() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let mut loopback = vec![IPV6];
    loopback.extend_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
    let (mut stream, reply) = connect_request(&front, &loopback, 22).await;
    assert_eq!(reply, NOT_ALLOWED);
    assert!(closed(&mut stream).await);
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn un_message_d_accueil_mal_forme_est_refuse_sans_ouvrir_de_canal() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let mut socks4 = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    socks4
        .write_all(&[0x04, 1, PASSWORD_AUTH])
        .await
        .expect("send a SOCKS4 greeting");
    assert!(
        closed(&mut socks4).await,
        "a SOCKS4 greeting gets no answer"
    );
    let mut no_method = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    no_method
        .write_all(&[SOCKS_VERSION, 0])
        .await
        .expect("send an empty method list");
    assert_eq!(
        read_array::<2>(&mut no_method).await,
        [SOCKS_VERSION, NO_ACCEPTABLE_METHOD]
    );
    assert!(closed(&mut no_method).await);
    let credentials = front.credentials();
    let mut wrong_version = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    wrong_version
        .write_all(&[SOCKS_VERSION, 1, PASSWORD_AUTH])
        .await
        .expect("send methods");
    assert_eq!(
        read_array::<2>(&mut wrong_version).await,
        [SOCKS_VERSION, PASSWORD_AUTH]
    );
    let mut request = vec![0x05, 64];
    request.extend_from_slice(credentials.username().as_bytes());
    request.push(64);
    request.extend_from_slice(credentials.password().as_bytes());
    wrong_version
        .write_all(&request)
        .await
        .expect("send credentials under a wrong sub-negotiation version");
    let [_, status] = read_array::<2>(&mut wrong_version).await;
    assert_ne!(status, SUCCEEDED);
    assert!(closed(&mut wrong_version).await);
    assert!(opener.opened().is_empty());
}

#[tokio::test]
async fn une_negociation_bloquee_est_coupee_apres_vingt_secondes() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let started = tokio::time::Instant::now();
    let mut stream = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    stream
        .write_all(&[SOCKS_VERSION, 1, PASSWORD_AUTH])
        .await
        .expect("send methods");
    let mut method = [0u8; 2];
    stream
        .read_exact(&mut method)
        .await
        .expect("read the method choice");
    assert_eq!(method, [SOCKS_VERSION, PASSWORD_AUTH]);
    tokio::time::pause();
    let mut buffer = [0u8; 1];
    let before_deadline = tokio::time::timeout(
        NEGOTIATION_TIMEOUT - Duration::from_secs(1),
        stream.read(&mut buffer),
    )
    .await;
    assert!(before_deadline.is_err(), "still open before the deadline");
    let after_deadline = stream.read(&mut buffer).await;
    assert!(
        matches!(after_deadline, Ok(0) | Err(_)),
        "closed at the deadline"
    );
    assert!(started.elapsed() >= NEGOTIATION_TIMEOUT);
    assert!(opener.opened().is_empty());
}

async fn greeted(front: &SocksFront) -> Option<TcpStream> {
    let mut stream = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    stream
        .write_all(&[SOCKS_VERSION, 1, PASSWORD_AUTH])
        .await
        .ok()?;
    let mut method = [0u8; 2];
    tokio::time::timeout(WAIT, stream.read_exact(&mut method))
        .await
        .ok()?
        .ok()?;
    (method == [SOCKS_VERSION, PASSWORD_AUTH]).then_some(stream)
}

#[tokio::test]
async fn les_negociations_en_attente_sont_bornees_et_l_excedent_est_ferme_aussitot() {
    let opener = Arc::new(CountingOpener::default());
    let front = front_with(opener.clone()).await;
    let mut pending = Vec::new();
    for _ in 0..PENDING_NEGOTIATIONS {
        pending.push(greeted(&front).await.expect("a pending negotiation"));
    }
    let mut excess = TcpStream::connect(front.address())
        .await
        .expect("reach the front");
    assert!(
        closed(&mut excess).await,
        "a connection beyond the pending limit is closed at once"
    );
    drop(pending);
    let mut served = None;
    for _ in 0..50 {
        served = greeted(&front).await;
        if served.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        served.is_some(),
        "the slots come back once the clients leave"
    );
    assert!(opener.opened().is_empty());
}
