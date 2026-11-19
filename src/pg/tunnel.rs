use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle, Msg};
use russh::keys::agent::client::AgentClient;
use russh::keys::{
    self, Algorithm, HashAlg, PrivateKeyWithHashAlg, PublicKey, PublicKeyOrCertificate,
};
use russh::{ChannelOpenFailure, ChannelStream};

use super::error::PgError;
use super::options::{Tunnel, TunnelAuth};

const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const ORIGINATOR_ADDRESS: &str = "127.0.0.1";

pub(crate) type TunnelStream = ChannelStream<Msg>;

pub(crate) struct SshLink {
    handle: Handle<HostKeyCheck>,
    host: String,
    port: u16,
    remote_host: String,
    remote_port: u16,
}

impl SshLink {
    pub(crate) async fn open(tunnel: &Tunnel) -> Result<Self, PgError> {
        let known_hosts = tunnel.verify_host_key.then(|| known_hosts_path(tunnel));
        let mut config = client::Config {
            keepalive_interval: Some(KEEPALIVE_INTERVAL),
            nodelay: true,
            ..client::Config::default()
        };
        if let Some(path) = &known_hosts {
            let recorded = recorded_algorithms(&tunnel.host, tunnel.port, path);
            config.preferred.key =
                Cow::Owned(known_algorithms_first(&config.preferred.key, &recorded));
        }
        let checker = HostKeyCheck {
            host: tunnel.host.clone(),
            port: tunnel.port,
            known_hosts,
        };
        let mut handle = client::connect(
            Arc::new(config),
            (tunnel.host.as_str(), tunnel.port),
            checker,
        )
        .await
        .map_err(|failure| failure.describe(&tunnel.host, tunnel.port))?;
        authenticate(&mut handle, tunnel).await?;
        Ok(Self {
            handle,
            host: tunnel.host.clone(),
            port: tunnel.port,
            remote_host: tunnel.remote_host.clone(),
            remote_port: tunnel.remote_port,
        })
    }

    pub(crate) async fn forward(&self) -> Result<TunnelStream, PgError> {
        let channel = self
            .handle
            .channel_open_direct_tcpip(
                self.remote_host.as_str(),
                u32::from(self.remote_port),
                ORIGINATOR_ADDRESS,
                0,
            )
            .await
            .map_err(|error| {
                let reason = match error {
                    russh::Error::ChannelOpenFailure(
                        ChannelOpenFailure::AdministrativelyProhibited,
                    ) => "forwarding is prohibited; enable AllowTcpForwarding on the SSH server"
                        .to_string(),
                    russh::Error::ChannelOpenFailure(ChannelOpenFailure::ConnectFailed) => {
                        "the SSH server could not reach the database address".to_string()
                    }
                    other => other.to_string(),
                };
                PgError::Connect(format!(
                    "SSH server {}:{} refused to forward to {}:{}: {reason}",
                    self.host, self.port, self.remote_host, self.remote_port
                ))
            })?;
        Ok(channel.into_stream())
    }

    pub(crate) fn remote_host(&self) -> &str {
        &self.remote_host
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }
}

fn known_hosts_path(tunnel: &Tunnel) -> PathBuf {
    tunnel
        .known_hosts
        .clone()
        .unwrap_or_else(default_known_hosts)
}

fn default_known_hosts() -> PathBuf {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().to_path_buf())
        .unwrap_or_default()
        .join(".ssh")
        .join("known_hosts")
}

fn recorded_algorithms(host: &str, port: u16, known_hosts: &Path) -> Vec<Algorithm> {
    keys::known_hosts::known_host_keys_path(host, port, known_hosts)
        .map(|keys| keys.into_iter().map(|(_, key)| key.algorithm()).collect())
        .unwrap_or_default()
}

fn known_algorithms_first(preferred: &[Algorithm], recorded: &[Algorithm]) -> Vec<Algorithm> {
    let is_recorded = |algorithm: &Algorithm| {
        recorded.iter().any(|known| {
            known == algorithm
                || matches!(
                    (known, algorithm),
                    (Algorithm::Rsa { .. }, Algorithm::Rsa { .. })
                )
        })
    };
    let (known, unknown): (Vec<Algorithm>, Vec<Algorithm>) =
        preferred.iter().cloned().partition(is_recorded);
    known.into_iter().chain(unknown).collect()
}

struct HostKeyCheck {
    host: String,
    port: u16,
    known_hosts: Option<PathBuf>,
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
    fn describe(self, host: &str, port: u16) -> PgError {
        PgError::Connect(match self {
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
        verify_host_key(&self.host, self.port, key, known_hosts)
            .map(|()| true)
            .map_err(SshFailure::HostKey)
    }
}

fn verify_host_key(
    host: &str,
    port: u16,
    key: &PublicKey,
    known_hosts: &Path,
) -> Result<(), String> {
    let fingerprint = Fingerprint(key);
    match keys::check_known_hosts_path(host, port, key, known_hosts) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "SSH host key of {host}:{port} ({fingerprint}) is not in known_hosts file {}; check this fingerprint with the server administrator, then add it with `ssh-keyscan -p {port} {host} >> {}`",
            known_hosts.display(),
            known_hosts.display()
        )),
        Err(keys::Error::KeyChanged { line }) => Err(format!(
            "SSH host key of {host}:{port} changed: the server now presents {fingerprint}, which differs from line {line} of {}; refusing to connect until the new key is confirmed with the server administrator and the old entry is replaced",
            known_hosts.display()
        )),
        Err(error) => Err(format!(
            "cannot check the SSH host key of {host}:{port} against {}: {error}",
            known_hosts.display()
        )),
    }
}

struct Fingerprint<'a>(&'a PublicKey);

impl fmt::Display for Fingerprint<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}",
            self.0.algorithm(),
            self.0.fingerprint(HashAlg::Sha256)
        )
    }
}

async fn authenticate(handle: &mut Handle<HostKeyCheck>, tunnel: &Tunnel) -> Result<(), PgError> {
    let location = format!("{}:{}", tunnel.host, tunnel.port);
    let protocol = |error: russh::Error| {
        PgError::Connect(format!(
            "SSH authentication with {location} failed: {error}"
        ))
    };
    let (method, accepted) = match &tunnel.auth {
        TunnelAuth::PublicKey {
            key_path,
            passphrase,
        } => {
            let key = load_key(key_path, passphrase.as_deref()).map_err(PgError::Connect)?;
            let hash = rsa_hash(handle, key.algorithm().is_rsa()).await;
            let key = PrivateKeyWithHashAlg::new(Arc::new(key), hash);
            let result = handle
                .authenticate_publickey(tunnel.user.as_str(), key)
                .await
                .map_err(protocol)?;
            (
                format!("public key {}", key_path.display()),
                result.success(),
            )
        }
        TunnelAuth::Password(password) => {
            let result = handle
                .authenticate_password(tunnel.user.as_str(), password.as_str())
                .await
                .map_err(protocol)?;
            ("password".to_string(), result.success())
        }
        TunnelAuth::Agent => (
            "SSH agent".to_string(),
            authenticate_with_agent(handle, tunnel).await?,
        ),
    };
    if accepted {
        Ok(())
    } else {
        Err(PgError::Connect(format!(
            "SSH server {location} rejected {method} authentication for user {}",
            tunnel.user
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
    tunnel: &Tunnel,
) -> Result<bool, PgError> {
    let unavailable = |error: keys::Error| {
        PgError::Connect(format!(
            "SSH agent unavailable: {error}; start an agent with the key loaded or pick key file authentication in DBeaver"
        ))
    };
    let mut agent = AgentClient::connect_env().await.map_err(unavailable)?;
    if let Some(binding) = handle.agent_session_binding() {
        let _ = agent.bind_session(binding, false).await;
    }
    let identities = agent.request_identities().await.map_err(unavailable)?;
    if identities.is_empty() {
        return Err(PgError::Connect(
            "SSH agent holds no key; add one with ssh-add".to_string(),
        ));
    }
    for identity in identities {
        let key = identity.public_key().into_owned();
        let hash = rsa_hash(handle, key.algorithm().is_rsa()).await;
        let result = handle
            .authenticate_publickey_with(tunnel.user.as_str(), key, hash, &mut agent)
            .await
            .map_err(|error| PgError::Connect(format!("SSH agent signing failed: {error}")))?;
        if result.success() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use russh::keys::EcdsaCurve;

    use super::*;

    #[test]
    fn host_key_types_recorded_in_known_hosts_are_offered_first() {
        let preferred = client::Config::default().preferred.key.into_owned();
        let p256 = Algorithm::Ecdsa {
            curve: EcdsaCurve::NistP256,
        };
        let ordered = known_algorithms_first(&preferred, std::slice::from_ref(&p256));
        assert_eq!(ordered.first(), Some(&p256));
        assert_eq!(ordered.len(), preferred.len());
        let ordered = known_algorithms_first(&preferred, &[Algorithm::Rsa { hash: None }]);
        assert!(
            ordered
                .iter()
                .take(3)
                .all(|algorithm| matches!(algorithm, Algorithm::Rsa { .. })),
            "{ordered:?}"
        );
        assert_eq!(known_algorithms_first(&preferred, &[]), preferred);
    }
}
