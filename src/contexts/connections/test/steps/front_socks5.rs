use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::domain::errors::SessionError;
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::socks::{
    AllowedTarget, ChannelOpener, SocksFront, TargetPolicy,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::net::TcpStream;

use crate::connections_world::ConnectionsWorld;

const VERSION: u8 = 0x05;
const NO_AUTH: u8 = 0x00;
const PASSWORD_AUTH: u8 = 0x02;
const NO_ACCEPTABLE_METHOD: u8 = 0xff;
const DOMAIN: u8 = 0x03;
const IPV4: u8 = 0x01;
const IPV6: u8 = 0x04;
const MEMBER_PORT: u16 = 27017;
const WAIT: Duration = Duration::from_secs(5);
const CHANNELS_PER_TARGET: usize = 2;

#[derive(Default)]
pub struct CountingOpener {
    opened: Mutex<Vec<(String, u16)>>,
    refuse: bool,
}

impl CountingOpener {
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

#[derive(Default)]
pub struct SocksState {
    policy: Option<Result<TargetPolicy, String>>,
    admitted: Option<Option<AllowedTarget>>,
    fronts: Vec<SocksFront>,
    opener: Arc<CountingOpener>,
    stream: Option<TcpStream>,
    reply: Option<u8>,
    refused_client: Option<bool>,
}

impl std::fmt::Debug for SocksState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SocksState")
            .field("admitted", &self.admitted)
            .field("reply", &self.reply)
            .finish_non_exhaustive()
    }
}

fn state(world: &mut ConnectionsWorld) -> &mut SocksState {
    world.socks.get_or_insert_with(SocksState::default)
}

#[given(regex = r#"^la liste d'admission du nom SRV "([^"]+)"$"#)]
async fn srv_policy(world: &mut ConnectionsWorld, name: String) {
    state(world).policy =
        Some(TargetPolicy::from_srv_name(&name).map_err(|error| error.to_string()));
}

#[when(regex = r#"^le serveur prépare la liste d'admission du nom SRV "([^"]+)"$"#)]
async fn prepare_srv_policy(world: &mut ConnectionsWorld, name: String) {
    state(world).policy =
        Some(TargetPolicy::from_srv_name(&name).map_err(|error| error.to_string()));
}

#[given(regex = r#"^la liste d'admission de l'hôte déclaré "([^"]+)"$"#)]
async fn seed_policy(world: &mut ConnectionsWorld, host: String) {
    state(world).policy =
        Some(TargetPolicy::from_seed_host(&host).map_err(|error| error.to_string()));
}

fn policy(world: &mut ConnectionsWorld) -> TargetPolicy {
    state(world)
        .policy
        .clone()
        .expect("a policy")
        .expect("a valid policy")
}

#[when(regex = r#"^le front reçoit une demande vers "([^"]+)" sur le port (\d+)$"#)]
async fn request_target(world: &mut ConnectionsWorld, host: String, port: u16) {
    let admitted = policy(world).admit(&host, port);
    state(world).admitted = Some(admitted);
}

#[when(regex = r"^le front reçoit une demande vers un membre dont un label compte 64 caractères$")]
async fn request_long_label(world: &mut ConnectionsWorld) {
    let host = format!("{}.example.com", "a".repeat(64));
    let admitted = policy(world).admit(&host, MEMBER_PORT);
    state(world).admitted = Some(admitted);
}

fn admitted(world: &mut ConnectionsWorld) -> Option<AllowedTarget> {
    state(world).admitted.clone().expect("a request was judged")
}

#[then(regex = r#"^la cible est admise sous le nom "([^"]+)"$"#)]
async fn target_admitted(world: &mut ConnectionsWorld, name: String) {
    let target = admitted(world).expect("the target is admitted");
    assert_eq!(target.host(), name);
}

#[then(regex = r"^la cible est refusée$")]
async fn target_refused(world: &mut ConnectionsWorld) {
    assert_eq!(admitted(world), None);
}

#[then(regex = r"^la liste d'admission est refusée avec le motif « (.+) »$")]
async fn policy_refused(world: &mut ConnectionsWorld, reason: String) {
    match state(world).policy.clone().expect("a policy") {
        Ok(_) => panic!("the policy must be refused"),
        Err(error) => assert_eq!(error, reason),
    }
}

async fn start_front(world: &mut ConnectionsWorld, member: &str, refuse: bool) {
    let state = state(world);
    if refuse {
        state.opener = Arc::new(CountingOpener {
            refuse: true,
            ..CountingOpener::default()
        });
    }
    let policy = TargetPolicy::from_seed_host(member).expect("valid member");
    let front = SocksFront::start(state.opener.clone(), policy, CHANNELS_PER_TARGET)
        .await
        .expect("start the SOCKS5 front");
    state.fronts.push(front);
}

#[given(regex = r#"^un front SOCKS5 démarré pour le membre "([^"]+)"$"#)]
async fn one_front(world: &mut ConnectionsWorld, member: String) {
    start_front(world, &member, false).await;
}

#[given(regex = r#"^deux fronts SOCKS5 démarrés pour le membre "([^"]+)"$"#)]
async fn two_fronts(world: &mut ConnectionsWorld, member: String) {
    start_front(world, &member, false).await;
    start_front(world, &member, false).await;
}

#[given(
    regex = r#"^un front SOCKS5 démarré pour le membre "([^"]+)" derrière un bastion qui refuse les canaux$"#
)]
async fn front_with_refusing_bastion(world: &mut ConnectionsWorld, member: String) {
    start_front(world, &member, true).await;
}

#[then(regex = r"^chaque front écoute sur 127.0.0.1$")]
async fn fronts_listen_on_loopback(world: &mut ConnectionsWorld) {
    let fronts = &state(world).fronts;
    assert!(!fronts.is_empty());
    for front in fronts {
        assert_eq!(front.address().ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(front.address().port(), 0);
    }
}

#[then(regex = r"^les deux fronts ont des identifiants différents$")]
async fn fronts_have_distinct_credentials(world: &mut ConnectionsWorld) {
    let fronts = &state(world).fronts;
    let [first, second] = fronts.as_slice() else {
        panic!("two fronts are expected");
    };
    let (one, two) = (first.credentials(), second.credentials());
    assert_ne!(one.username(), two.username());
    assert_ne!(one.password(), two.password());
    assert_ne!(one.username(), one.password());
}

async fn read_exact<const N: usize>(stream: &mut TcpStream) -> Option<[u8; N]> {
    let mut buffer = [0u8; N];
    match tokio::time::timeout(WAIT, stream.read_exact(&mut buffer)).await {
        Ok(Ok(_)) => Some(buffer),
        _ => None,
    }
}

async fn authenticate(stream: &mut TcpStream, username: &str, password: &str) -> u8 {
    stream
        .write_all(&[VERSION, 1, PASSWORD_AUTH])
        .await
        .expect("send methods");
    assert_eq!(
        read_exact::<2>(stream).await,
        Some([VERSION, PASSWORD_AUTH])
    );
    let mut request = vec![1, u8::try_from(username.len()).expect("short user")];
    request.extend_from_slice(username.as_bytes());
    request.push(u8::try_from(password.len()).expect("short password"));
    request.extend_from_slice(password.as_bytes());
    stream.write_all(&request).await.expect("send credentials");
    read_exact::<2>(stream)
        .await
        .map_or(0xff, |[_, status]| status)
}

async fn closed(stream: &mut TcpStream) -> bool {
    let mut buffer = [0u8; 1];
    matches!(
        tokio::time::timeout(WAIT, stream.read(&mut buffer)).await,
        Ok(Ok(0) | Err(_))
    )
}

#[when(regex = r"^un client qui ne propose pas d'authentification se présente au front$")]
async fn client_without_authentication(world: &mut ConnectionsWorld) {
    let address = state(world).fronts[0].address();
    let mut stream = TcpStream::connect(address).await.expect("reach the front");
    stream
        .write_all(&[VERSION, 1, NO_AUTH])
        .await
        .expect("send methods");
    let refused = read_exact::<2>(&mut stream).await == Some([VERSION, NO_ACCEPTABLE_METHOD])
        && closed(&mut stream).await;
    state(world).refused_client = Some(refused);
}

#[when(regex = r"^un client qui présente d'autres identifiants se présente au front$")]
async fn client_with_other_credentials(world: &mut ConnectionsWorld) {
    let front = &state(world).fronts[0];
    let address = front.address();
    let username = front.credentials().username().to_string();
    let mut stream = TcpStream::connect(address).await.expect("reach the front");
    let status = authenticate(&mut stream, &username, "not-the-session-password").await;
    let refused = status != 0 && closed(&mut stream).await;
    state(world).refused_client = Some(refused);
}

#[then(regex = r"^le front refuse le client$")]
async fn client_refused(world: &mut ConnectionsWorld) {
    assert_eq!(state(world).refused_client, Some(true));
}

fn address_of(kind: &str, value: &str) -> Vec<u8> {
    match kind {
        "le nom" => {
            let mut address = vec![DOMAIN, u8::try_from(value.len()).expect("short name")];
            address.extend_from_slice(value.as_bytes());
            address
        }
        "l'adresse IPv4" => {
            let ip: Ipv4Addr = value.parse().expect("an IPv4 address");
            let mut address = vec![IPV4];
            address.extend_from_slice(&ip.octets());
            address
        }
        _ => {
            let ip: Ipv6Addr = value.parse().expect("an IPv6 address");
            let mut address = vec![IPV6];
            address.extend_from_slice(&ip.octets());
            address
        }
    }
}

#[when(
    regex = r#"^un client authentifié demande (CONNECT|BIND|UDP ASSOCIATE) vers (le nom|l'adresse IPv4|l'adresse IPv6) "?([^"]+)"?$"#
)]
async fn authenticated_request(
    world: &mut ConnectionsWorld,
    command: String,
    kind: String,
    target: String,
) {
    let front = &state(world).fronts[0];
    let address = front.address();
    let (username, password) = (
        front.credentials().username().to_string(),
        front.credentials().password().to_string(),
    );
    let mut stream = TcpStream::connect(address).await.expect("reach the front");
    assert_eq!(authenticate(&mut stream, &username, &password).await, 0);
    let code = match command.as_str() {
        "CONNECT" => 0x01,
        "BIND" => 0x02,
        _ => 0x03,
    };
    let mut request = vec![VERSION, code, 0];
    request.extend(address_of(&kind, &target));
    request.extend_from_slice(&MEMBER_PORT.to_be_bytes());
    stream.write_all(&request).await.expect("send the request");
    let reply = read_exact::<10>(&mut stream).await.expect("a SOCKS5 reply");
    assert_eq!(reply[0], VERSION);
    let state = state(world);
    state.reply = Some(reply[1]);
    state.stream = Some(stream);
}

#[then(regex = r"^le front répond par un succès$")]
async fn reply_success(world: &mut ConnectionsWorld) {
    assert_eq!(state(world).reply, Some(0x00));
}

#[then(regex = r"^le front répond par le refus « (.+) »$")]
async fn reply_refusal(world: &mut ConnectionsWorld, refusal: String) {
    let expected = match refusal.as_str() {
        "échec général" => 0x01,
        "connexion non autorisée" => 0x02,
        "commande non prise en charge" => 0x07,
        other => panic!("unknown refusal {other}"),
    };
    let state = state(world);
    assert_eq!(state.reply, Some(expected));
    let stream = state.stream.as_mut().expect("a client stream");
    assert!(closed(stream).await, "the front closes the refused request");
}

#[then(regex = r"^aucun canal n'a été ouvert vers le bastion$")]
async fn no_channel(world: &mut ConnectionsWorld) {
    assert!(state(world).opener.opened().is_empty());
}

#[then(regex = r#"^un canal a été ouvert vers "([^"]+)" sur le port (\d+)$"#)]
async fn channel_opened(world: &mut ConnectionsWorld, host: String, port: u16) {
    assert_eq!(state(world).opener.opened(), vec![(host, port)]);
}

#[then(regex = r"^les octets envoyés par le client lui reviennent par ce canal$")]
async fn bytes_echo(world: &mut ConnectionsWorld) {
    let stream = state(world).stream.as_mut().expect("a client stream");
    stream.write_all(b"ping").await.expect("write through");
    assert_eq!(read_exact::<4>(stream).await, Some(*b"ping"));
}
