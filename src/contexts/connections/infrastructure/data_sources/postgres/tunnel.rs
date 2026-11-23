use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle, Msg};
use russh::keys::agent::client::AgentClient;
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelOpenFailure, ChannelStream};

use super::known_hosts::{
    home_dir, known_algorithms_first, known_hosts_path, lookup_name, recorded_algorithms,
    verify_host_key,
};
use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::{Hop, OpenSshKeys, SshAgent, Tunnel, TunnelAuth};

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const ORIGINATOR_ADDRESS: &str = "127.0.0.1";

pub(crate) type TunnelStream = ChannelStream<Msg>;

pub(crate) struct SshLink {
    handles: Vec<Handle<HostKeyCheck>>,
    host: String,
    port: u16,
    remote_host: String,
    remote_port: u16,
}

struct Position<'a> {
    index: usize,
    total: usize,
    hop: &'a Hop,
}

impl Position<'_> {
    fn tag(&self, error: SessionError) -> SessionError {
        if self.total == 1 {
            return error;
        }
        let prefix = format!(
            "SSH hop {} of {} ({})",
            self.index + 1,
            self.total,
            self.hop.label
        );
        match error {
            SessionError::Connect(text) => SessionError::Connect(format!("{prefix}: {text}")),
            other => other,
        }
    }
}

impl SshLink {
    pub(crate) async fn open(tunnel: &Tunnel) -> Result<Self, SessionError> {
        let total = tunnel.hops.len();
        let mut handles: Vec<Handle<HostKeyCheck>> = Vec::with_capacity(total);
        for (index, hop) in tunnel.hops.iter().enumerate() {
            let position = Position { index, total, hop };
            let mut handle = connect_hop(handles.last(), &position).await?;
            authenticate(&mut handle, hop)
                .await
                .map_err(|error| position.tag(error))?;
            handles.push(handle);
        }
        let last = tunnel.hops.last().ok_or_else(|| {
            SessionError::Connect("the SSH tunnel has no hop to connect through".to_string())
        })?;
        Ok(Self {
            handles,
            host: last.host.clone(),
            port: last.port,
            remote_host: tunnel.remote_host.clone(),
            remote_port: tunnel.remote_port,
        })
    }

    pub(crate) async fn forward(&self) -> Result<TunnelStream, SessionError> {
        let last = self.handles.last().ok_or_else(|| {
            SessionError::Connect("the SSH tunnel has no hop to forward through".to_string())
        })?;
        let channel = last
            .channel_open_direct_tcpip(
                self.remote_host.as_str(),
                u32::from(self.remote_port),
                ORIGINATOR_ADDRESS,
                0,
            )
            .await
            .map_err(|error| {
                forward_refusal(
                    error,
                    &self.host,
                    self.port,
                    &self.remote_host,
                    self.remote_port,
                )
            })?;
        Ok(channel.into_stream())
    }

    pub(crate) fn remote_host(&self) -> &str {
        &self.remote_host
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.handles.iter().any(Handle::is_closed)
    }
}

fn forward_refusal(
    error: russh::Error,
    host: &str,
    port: u16,
    remote_host: &str,
    remote_port: u16,
) -> SessionError {
    let reason = match error {
        russh::Error::ChannelOpenFailure(ChannelOpenFailure::AdministrativelyProhibited) => {
            "forwarding is prohibited; enable AllowTcpForwarding on the SSH server".to_string()
        }
        russh::Error::ChannelOpenFailure(ChannelOpenFailure::ConnectFailed) => {
            "the SSH server could not reach the database address".to_string()
        }
        other => other.to_string(),
    };
    SessionError::Connect(format!(
        "SSH server {host}:{port} refused to forward to {remote_host}:{remote_port}: {reason}"
    ))
}

async fn connect_hop(
    previous: Option<&Handle<HostKeyCheck>>,
    position: &Position<'_>,
) -> Result<Handle<HostKeyCheck>, SessionError> {
    let hop = position.hop;
    let known_hosts = if hop.verify_host_key {
        let files = if hop.known_hosts.is_empty() {
            vec![known_hosts_path(None, home_dir()).map_err(SessionError::Connect)?]
        } else {
            hop.known_hosts.clone()
        };
        Some(files)
    } else {
        None
    };
    let mut config = client::Config {
        keepalive_interval: Some(KEEPALIVE_INTERVAL),
        nodelay: true,
        ..client::Config::default()
    };
    if let Some(files) = &known_hosts {
        let name = lookup_name(&hop.host, hop.port, hop.host_key_alias.as_deref());
        let recorded = recorded_algorithms(&name, files);
        config.preferred.key = Cow::Owned(known_algorithms_first(&config.preferred.key, &recorded));
    }
    let checker = HostKeyCheck {
        host: hop.host.clone(),
        port: hop.port,
        alias: hop.host_key_alias.clone(),
        known_hosts,
    };
    let config = Arc::new(config);
    let connected = match previous {
        None => client::connect(config, (hop.host.as_str(), hop.port), checker).await,
        Some(previous) => {
            let channel = previous
                .channel_open_direct_tcpip(
                    hop.host.as_str(),
                    u32::from(hop.port),
                    ORIGINATOR_ADDRESS,
                    0,
                )
                .await
                .map_err(|error| {
                    position.tag(forward_refusal(
                        error, &hop.host, hop.port, &hop.host, hop.port,
                    ))
                })?;
            client::connect_stream(config, channel.into_stream(), checker).await
        }
    };
    connected.map_err(|failure| position.tag(failure.describe(&hop.host, hop.port)))
}

struct HostKeyCheck {
    host: String,
    port: u16,
    alias: Option<String>,
    known_hosts: Option<Vec<PathBuf>>,
}

#[derive(Debug)]
enum SshFailure {
    Protocol(russh::Error),
    HostKey(String),
}

impl From<russh::Error> for SshFailure {
    fn from(error: russh::Error) -> Self {
        Self::Protocol(error)
    }
}

impl SshFailure {
    fn describe(self, host: &str, port: u16) -> SessionError {
        SessionError::Connect(match self {
            Self::HostKey(message) => message,
            Self::Protocol(russh::Error::IO(error)) => {
                format!("cannot reach SSH server {host}:{port}: {error}")
            }
            Self::Protocol(russh::Error::Disconnect | russh::Error::HUP) => format!(
                "SSH server {host}:{port} closed the connection before authentication; it may be limiting concurrent connections or refusing this client"
            ),
            Self::Protocol(error) => format!("SSH connection to {host}:{port} failed: {error}"),
        })
    }
}

impl client::Handler for HostKeyCheck {
    type Error = SshFailure;

    async fn check_server_key(
        &mut self,
        server_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let Some(known_hosts) = &self.known_hosts else {
            return Ok(true);
        };
        let PublicKeyOrCertificate::PublicKey { key, .. } = server_key else {
            return Err(SshFailure::HostKey(format!(
                "SSH server {}:{} presented a host certificate; only host keys listed in known_hosts are supported",
                self.host, self.port
            )));
        };
        verify_host_key(
            &self.host,
            self.port,
            self.alias.as_deref(),
            key,
            known_hosts,
        )
        .map(|()| true)
        .map_err(SshFailure::HostKey)
    }
}

async fn authenticate(handle: &mut Handle<HostKeyCheck>, hop: &Hop) -> Result<(), SessionError> {
    let location = format!("{}:{}", hop.host, hop.port);
    let protocol = |error: russh::Error| {
        SessionError::Connect(format!(
            "SSH authentication with {location} failed: {error}"
        ))
    };
    let (method, accepted) = match &hop.auth {
        TunnelAuth::PublicKey {
            key_path,
            passphrase,
        } => {
            let key = load_key(key_path, passphrase.as_deref()).map_err(SessionError::Connect)?;
            let hash = rsa_hash(handle, key.algorithm().is_rsa()).await;
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
            let result = handle
                .authenticate_publickey(hop.user.as_str(), key)
                .await
                .map_err(protocol)?;
            (
                format!("public key {}", key_path.display()),
                result.success(),
            )
        }
        TunnelAuth::Password(password) => {
            let result = handle
                .authenticate_password(hop.user.as_str(), password.as_str())
                .await
                .map_err(protocol)?;
            ("password".to_string(), result.success())
        }
        TunnelAuth::Agent => (
            "SSH agent".to_string(),
            authenticate_with_agent(handle, hop).await?,
        ),
        TunnelAuth::OpenSsh(keys) => return authenticate_with_keys(handle, hop, keys).await,
    };
    if accepted {
        Ok(())
    } else {
        Err(SessionError::Connect(format!(
            "SSH server {location} rejected {method} authentication for user {}",
            hop.user
        )))
    }
}

async fn rsa_hash(handle: &Handle<HostKeyCheck>, is_rsa: bool) -> Option<HashAlg> {
    if !is_rsa {
        return None;
    }
    handle
        .best_supported_rsa_hash()
        .await
        .ok()
        .flatten()
        .flatten()
}

fn load_key(path: &Path, passphrase: Option<&str>) -> Result<keys::PrivateKey, String> {
    keys::load_secret_key(path, passphrase).map_err(|error| {
        let path = path.display();
        match (error, passphrase) {
            (keys::Error::KeyIsEncrypted, _) => format!(
                "SSH private key {path} is encrypted but no passphrase is configured; save the key passphrase in the DBeaver connection"
            ),
            (keys::Error::IO(error), _) => format!("cannot read SSH private key {path}: {error}"),
            (_, Some(_)) => format!(
                "cannot decrypt SSH private key {path}: the passphrase is wrong or the key format is unsupported"
            ),
            (error, None) => format!("cannot load SSH private key {path}: {error}"),
        }
    })
}

async fn authenticate_with_agent(
    handle: &mut Handle<HostKeyCheck>,
    hop: &Hop,
) -> Result<bool, SessionError> {
    let unavailable = |error: keys::Error| {
        SessionError::Connect(format!(
            "SSH agent unavailable: {error}; start an agent with the key loaded or pick key file authentication in DBeaver"
        ))
    };
    let mut agent = AgentClient::connect_env().await.map_err(unavailable)?;
    if let Some(binding) = handle.agent_session_binding() {
        let _ = agent.bind_session(binding, false).await;
    }
    let identities = agent.request_identities().await.map_err(unavailable)?;
    if identities.is_empty() {
        return Err(SessionError::Connect(
            "SSH agent holds no key; add one with ssh-add".to_string(),
        ));
    }
    for identity in identities {
        let key = identity.public_key().into_owned();
        let hash = rsa_hash(handle, key.algorithm().is_rsa()).await;
        let result = handle
            .authenticate_publickey_with(hop.user.as_str(), key, hash, &mut agent)
            .await
            .map_err(|error| SessionError::Connect(format!("SSH agent signing failed: {error}")))?;
        if result.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn authenticate_with_keys(
    handle: &mut Handle<HostKeyCheck>,
    hop: &Hop,
    keys: &OpenSshKeys,
) -> Result<(), SessionError> {
    let location = format!("{}:{}", hop.host, hop.port);
    let failed = |error: russh::Error| {
        SessionError::Connect(format!(
            "SSH authentication with {location} failed: {error}"
        ))
    };
    let allowed = keys
        .identities_only
        .then(|| public_keys_of(&keys.identity_files));
    if let Some(mut agent) = connect_agent(&keys.agent).await {
        if let Some(binding) = handle.agent_session_binding() {
            let _ = agent.bind_session(binding, false).await;
        }
        for identity in agent.request_identities().await.unwrap_or_default() {
            let key = identity.public_key().into_owned();
            let offered = allowed.as_ref().is_none_or(|allowed| {
                allowed
                    .iter()
                    .any(|wanted| wanted.key_data() == key.key_data())
            });
            if !offered {
                continue;
            }
            let hash = rsa_hash(handle, key.algorithm().is_rsa()).await;
            let result = handle
                .authenticate_publickey_with(hop.user.as_str(), key, hash, &mut agent)
                .await
                .map_err(|error| {
                    SessionError::Connect(format!("SSH agent signing failed: {error}"))
                })?;
            if result.success() {
                return Ok(());
            }
        }
    }
    let mut encrypted: Option<&Path> = None;
    for path in &keys.identity_files {
        match keys::load_secret_key(path, None) {
            Ok(key) => {
                let hash = rsa_hash(handle, key.algorithm().is_rsa()).await;
                let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
                let result = handle
                    .authenticate_publickey(hop.user.as_str(), key)
                    .await
                    .map_err(failed)?;
                if result.success() {
                    return Ok(());
                }
            }
            Err(keys::Error::KeyIsEncrypted) => encrypted = encrypted.or(Some(path)),
            Err(_) => {}
        }
    }
    Err(SessionError::Connect(match encrypted {
        Some(path) => format!(
            "SSH private key {} is encrypted and no agent offers it; load it into the SSH agent with ssh-add",
            path.display()
        ),
        None => format!(
            "SSH server {location} accepted none of the available keys for user {}; load a key into the SSH agent or name an unencrypted IdentityFile",
            hop.user
        ),
    }))
}

async fn connect_agent(agent: &SshAgent) -> Option<AgentClient<tokio::net::UnixStream>> {
    match agent {
        SshAgent::Disabled => None,
        SshAgent::Environment => AgentClient::connect_env().await.ok(),
        SshAgent::Socket(path) => AgentClient::connect_uds(path).await.ok(),
    }
}

fn public_keys_of(identity_files: &[PathBuf]) -> Vec<keys::PublicKey> {
    identity_files
        .iter()
        .filter_map(|path| {
            let mut companion = path.clone().into_os_string();
            companion.push(".pub");
            keys::load_public_key(&companion).ok().or_else(|| {
                keys::load_secret_key(path, None)
                    .ok()
                    .map(|key| key.public_key().clone())
            })
        })
        .collect()
}
