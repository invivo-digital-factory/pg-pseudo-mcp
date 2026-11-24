use std::collections::VecDeque;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use fast_socks5::Socks5Command;
use fast_socks5::consts::{
    SOCKS5_ADDR_TYPE_IPV4, SOCKS5_REPLY_CONNECTION_NOT_ALLOWED, SOCKS5_REPLY_GENERAL_FAILURE,
    SOCKS5_REPLY_SUCCEEDED, SOCKS5_VERSION,
};
use fast_socks5::server::{Config, DenyAuthentication, Socks5Socket};
use fast_socks5::util::target_addr::TargetAddr;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::{AbortHandle, JoinHandle, JoinSet};

use super::authentication::authenticate;
use super::channel_opener::ChannelOpener;
use super::credentials::ProxyCredentials;
use super::target_policy::{AllowedTarget, TargetPolicy};

const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(2);
const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(20);
const ACCEPT_BACKOFF: Duration = Duration::from_millis(100);
const PENDING_HANDSHAKES: usize = 32;

type RequestConfig = Config<DenyAuthentication>;

pub struct SocksFront {
    address: SocketAddr,
    credentials: ProxyCredentials,
    serving: JoinHandle<()>,
}

struct Relay<C> {
    credentials: ProxyCredentials,
    config: Arc<RequestConfig>,
    opener: Arc<C>,
    policy: TargetPolicy,
}

impl SocksFront {
    pub async fn start<C: ChannelOpener>(opener: Arc<C>, policy: TargetPolicy) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let credentials = ProxyCredentials::generate()?;
        let relay = Arc::new(Relay {
            credentials: credentials.clone(),
            config: Arc::new(request_config()),
            opener,
            policy,
        });
        let serving = tokio::spawn(serve(listener, relay));
        Ok(Self {
            address,
            credentials,
            serving,
        })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn credentials(&self) -> &ProxyCredentials {
        &self.credentials
    }
}

impl Drop for SocksFront {
    fn drop(&mut self) {
        self.serving.abort();
    }
}

fn request_config() -> RequestConfig {
    let mut config = RequestConfig::default();
    config
        .set_skip_auth(true)
        .set_execute_command(false)
        .set_dns_resolve(false);
    config
}

async fn serve<C: ChannelOpener>(listener: TcpListener, relay: Arc<Relay<C>>) {
    let mut handshakes = JoinSet::new();
    let mut oldest_first: VecDeque<AbortHandle> = VecDeque::new();
    let mut relays = JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => {
                    if oldest_first.len() >= PENDING_HANDSHAKES
                        && let Some(oldest) = oldest_first.pop_front()
                    {
                        oldest.abort();
                    }
                    let credentials = relay.credentials.clone();
                    oldest_first.push_back(handshakes.spawn(handshake(stream, credentials)));
                }
                Err(_) => tokio::time::sleep(ACCEPT_BACKOFF).await,
            },
            Some(finished) = handshakes.join_next_with_id(), if !handshakes.is_empty() => {
                let id = match &finished {
                    Ok((id, _)) => *id,
                    Err(error) => error.id(),
                };
                oldest_first.retain(|handle| handle.id() != id);
                if let Ok((_, Some(stream))) = finished {
                    relays.spawn(relay_connection(stream, relay.clone()));
                }
            }
            Some(_) = relays.join_next(), if !relays.is_empty() => {}
        }
    }
}

async fn handshake(mut stream: TcpStream, credentials: ProxyCredentials) -> Option<TcpStream> {
    let _ = stream.set_nodelay(true);
    let accepted = tokio::time::timeout(
        AUTHENTICATION_TIMEOUT,
        authenticate(&mut stream, &credentials),
    )
    .await;
    matches!(accepted, Ok(Ok(true))).then_some(stream)
}

async fn relay_connection<C: ChannelOpener>(stream: TcpStream, relay: Arc<Relay<C>>) {
    let negotiated = tokio::time::timeout(NEGOTIATION_TIMEOUT, negotiate(stream, &relay)).await;
    if let Ok(Some((mut client, mut channel))) = negotiated {
        let _ = tokio::io::copy_bidirectional(&mut client, &mut channel).await;
    }
}

async fn negotiate<C: ChannelOpener>(
    stream: TcpStream,
    relay: &Relay<C>,
) -> Option<(TcpStream, C::Stream)> {
    let socket = Socks5Socket::new(stream, relay.config.clone())
        .upgrade_to_socks5()
        .await
        .ok()?;
    let target = admitted(&socket, &relay.policy);
    let mut client = socket.into_inner();
    let Some(target) = target else {
        let _ = reply(&mut client, SOCKS5_REPLY_CONNECTION_NOT_ALLOWED).await;
        return None;
    };
    match relay.opener.open_channel(&target).await {
        Ok(channel) => {
            reply(&mut client, SOCKS5_REPLY_SUCCEEDED).await.ok()?;
            Some((client, channel))
        }
        Err(_) => {
            let _ = reply(&mut client, SOCKS5_REPLY_GENERAL_FAILURE).await;
            None
        }
    }
}

fn admitted(
    socket: &Socks5Socket<TcpStream, DenyAuthentication>,
    policy: &TargetPolicy,
) -> Option<AllowedTarget> {
    if socket.cmd() != &Some(Socks5Command::TCPConnect) {
        return None;
    }
    match socket.target_addr()? {
        TargetAddr::Domain(host, port) => policy.admit(host, *port),
        TargetAddr::Ip(_) => None,
    }
}

async fn reply(client: &mut TcpStream, code: u8) -> io::Result<()> {
    client
        .write_all(&[
            SOCKS5_VERSION,
            code,
            0x00,
            SOCKS5_ADDR_TYPE_IPV4,
            0,
            0,
            0,
            0,
            0,
            0,
        ])
        .await?;
    client.flush().await
}
