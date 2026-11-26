use crate::contexts::connections::domain::types::HeavyQueryLimits;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::credentials::CredentialStore;
use super::variables::{process_env, resolve};
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::services::is_azure_postgres_host;
use crate::contexts::connections::domain::types::{
    ConnectParams, ConnectionEntry, ConnectionSource, ConnectionStatus, HostKeyPolicy,
    PasswordSource, Secret, SshAuth, SshHop, SslMode, TunnelParams,
};
use crate::contexts::connections::infrastructure::data_sources::password_resolution::{
    SignIn, pgpass_or_azure_token,
};
use crate::contexts::connections::infrastructure::data_sources::pgpass;

pub(super) static NULL: Value = Value::Null;

const JUMP_SERVER_REASON: &str = "SSH jump servers are not supported yet";
pub(super) const SSH_USER_REASON: &str = "SSH user not saved in DBeaver";
const SSH_PASSWORD_REASON: &str = "no saved SSH password for this connection in DBeaver";
const NO_USER_REASON: &str = "no saved user name for this connection in DBeaver";
const PROXY_REASON: &str = "proxy not supported in this version";
const MASTER_PASSWORD_REASON: &str = "credentials protected by DBeaver master password or secure storage; unlock or re-save them in DBeaver";
const NO_PASSWORD_REASON: &str = "no saved password for this connection in DBeaver";
pub(super) const UNREADABLE_REASON: &str = "data-sources file is not valid JSON (the DBeaver project may be encrypted); its connections cannot be listed";

pub fn load(
    root: &Path,
    tokens: &dyn AzureSignInTokenProvider,
    pgpass_file: Option<&Path>,
    database: Option<&str>,
) -> Vec<ConnectionEntry> {
    let lookup = Lookup {
        sign_in: SignIn::new(tokens),
        pgpass_file,
        database,
    };
    let mut entries = Vec::new();
    for (project, dir) in project_dirs(root) {
        for (suffix, path) in data_source_files(&dir) {
            entries.extend(load_file(&project, &dir, &suffix, &path, &lookup));
        }
    }
    entries
}

pub(super) fn project_dirs(root: &Path) -> Vec<(String, PathBuf)> {
    let mut projects: Vec<(String, PathBuf)> = fs::read_dir(root.join("workspace6"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.path().join(".dbeaver"),
            )
        })
        .filter(|(_, dir)| dir.is_dir())
        .collect();
    projects.sort();
    projects
}

pub(super) fn data_source_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut files: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let suffix = name
                .strip_prefix("data-sources")?
                .strip_suffix(".json")?
                .to_string();
            (suffix.is_empty() || suffix.starts_with('-')).then(|| (suffix, entry.path()))
        })
        .collect();
    files.sort();
    files
}

struct Lookup<'a> {
    sign_in: SignIn<'a>,
    pgpass_file: Option<&'a Path>,
    database: Option<&'a str>,
}

fn load_file(
    project: &str,
    dir: &Path,
    suffix: &str,
    path: &Path,
    lookup: &Lookup<'_>,
) -> Vec<ConnectionEntry> {
    let file_label = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parsed = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(Value::is_object);
    let Some(root) = parsed else {
        return vec![unreadable_entry(project, &file_label)];
    };
    let (store, credentials_problem) = load_credentials(dir, suffix);
    let Some(connections) = root.get("connections").and_then(Value::as_object) else {
        return Vec::new();
    };
    connections
        .iter()
        .map(|(id, connection)| {
            build_entry(
                project,
                id,
                connection,
                &store,
                credentials_problem.as_deref(),
                lookup,
            )
        })
        .collect()
}

fn unreadable_entry(project: &str, file_label: &str) -> ConnectionEntry {
    let label = format!("{project}/{file_label}");
    ConnectionEntry {
        id: label.clone(),
        declared_name: label.clone(),
        source: ConnectionSource::Dbeaver,
        name: label,
        folder: None,
        project: project.to_string(),
        engine: "unknown".to_string(),
        connection_type: "unknown".to_string(),
        heavy_query: HeavyQueryLimits::default(),
        database: None,
        status: ConnectionStatus::Unavailable {
            reason: UNREADABLE_REASON.to_string(),
        },
        params: None,
        mongo: None,
    }
}

pub(super) fn load_credentials(dir: &Path, suffix: &str) -> (CredentialStore, Option<String>) {
    let path = dir.join(format!("credentials-config{suffix}.json"));
    let Ok(bytes) = fs::read(&path) else {
        return (CredentialStore::default(), None);
    };
    match CredentialStore::from_encrypted(&bytes) {
        Ok(store) => (store, None),
        Err(error) => (CredentialStore::default(), Some(error.to_string())),
    }
}

pub(super) fn text(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn build_entry(
    project: &str,
    id: &str,
    connection: &Value,
    store: &CredentialStore,
    credentials_problem: Option<&str>,
    lookup: &Lookup<'_>,
) -> ConnectionEntry {
    let configuration = connection.get("configuration").unwrap_or(&NULL);
    let engine = text(connection, "provider").unwrap_or_else(|| "unknown".to_string());
    let connection_type = text(configuration, "type").unwrap_or_else(|| "dev".to_string());
    let database = text(configuration, "database").filter(|value| !value.contains("${"));
    let (status, params) = match evaluate(
        id,
        &engine,
        connection,
        configuration,
        store,
        credentials_problem,
        lookup,
    ) {
        Ok(params) => (ConnectionStatus::Available, Some(params)),
        Err(status) => (status, None),
    };
    let database = params
        .as_ref()
        .map(|params| params.database.clone())
        .or(database);
    let name = text(connection, "name").unwrap_or_else(|| id.to_string());
    ConnectionEntry {
        id: id.to_string(),
        declared_name: name.clone(),
        source: ConnectionSource::Dbeaver,
        name,
        folder: text(connection, "folder"),
        project: project.to_string(),
        engine,
        connection_type,
        heavy_query: HeavyQueryLimits::default(),
        database,
        status,
        params,
        mongo: None,
    }
}

fn unavailable(reason: impl Into<String>) -> ConnectionStatus {
    ConnectionStatus::Unavailable {
        reason: reason.into(),
    }
}

fn unsupported(reason: impl Into<String>) -> ConnectionStatus {
    ConnectionStatus::Unsupported {
        reason: reason.into(),
    }
}

pub(super) struct Endpoint {
    pub host: String,
    pub port: u16,
    pub database: Option<String>,
    pub sslmode: SslMode,
}

pub(super) fn pgpass_auth_model(configuration: &Value) -> Result<bool, ConnectionStatus> {
    match text(configuration, "auth-model").as_deref() {
        None | Some("native") => Ok(false),
        Some("postgres_pgpass") => Ok(true),
        Some(model) => Err(unsupported(format!(
            "authentication model {model} is not supported in this version"
        ))),
    }
}

pub(super) fn expander(
    configuration: &Value,
) -> impl Fn(&str) -> Result<String, ConnectionStatus> + '_ {
    move |input: &str| {
        let own_field = |name: &str| match name {
            "host" | "port" | "database" | "user" => text(configuration, name),
            _ => None,
        };
        resolve(input, own_field, process_env).map_err(unavailable)
    }
}

pub(super) fn endpoint<E>(configuration: &Value, expand: &E) -> Result<Endpoint, ConnectionStatus>
where
    E: Fn(&str) -> Result<String, ConnectionStatus>,
{
    let mut sslmode_raw: Option<String> = None;
    let (host, port, database) =
        if text(configuration, "configurationType").as_deref() == Some("URL") {
            let url = text(configuration, "url")
                .ok_or_else(|| unavailable("connection is in URL mode but has no url"))?;
            let parts = parse_jdbc_url(&expand(&url)?).map_err(unavailable)?;
            sslmode_raw = parts
                .query
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("sslmode"))
                .map(|(_, value)| value.clone());
            (parts.host, parts.port, parts.database)
        } else {
            let host = text(configuration, "host")
                .map(|value| expand(&value))
                .transpose()?
                .ok_or_else(|| unavailable("connection has no host"))?;
            let port = text(configuration, "port")
                .map(|value| expand(&value))
                .transpose()?
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| unavailable("connection port is not a valid number"))
                })
                .transpose()?;
            let database = text(configuration, "database")
                .map(|value| expand(&value))
                .transpose()?;
            (host, port, database)
        };
    let sslmode = parse_sslmode(
        sslmode_raw
            .or_else(|| property_sslmode(configuration.get("provider-properties")))
            .or_else(|| property_sslmode(configuration.get("properties")))
            .as_deref(),
    )?;
    Ok(Endpoint {
        host,
        port: port.unwrap_or(5432),
        database,
        sslmode,
    })
}

fn evaluate(
    id: &str,
    engine: &str,
    connection: &Value,
    configuration: &Value,
    store: &CredentialStore,
    credentials_problem: Option<&str>,
    lookup: &Lookup<'_>,
) -> Result<ConnectParams, ConnectionStatus> {
    if engine != "postgresql" {
        return Err(unsupported(format!(
            "engine {engine} is not supported, only PostgreSQL"
        )));
    }
    let tunnel_handler = find_tunnel_handler(configuration)?;
    let pgpass_auth = pgpass_auth_model(configuration)?;
    let expand = expander(configuration);
    let endpoint = endpoint(configuration, &expand)?;
    let (host, port, sslmode) = (endpoint.host, endpoint.port, endpoint.sslmode);
    let database = lookup
        .database
        .map(str::to_string)
        .or(endpoint.database)
        .unwrap_or_else(|| "postgres".to_string());
    let saved = store.get(id);
    let plain_user = text(configuration, "user")
        .map(|value| expand(&value))
        .transpose()?;
    let user = plain_user
        .or_else(|| saved.and_then(|saved| saved.user.clone()))
        .filter(|user| !user.is_empty());

    let (user, password) = if pgpass_auth {
        let user = user.ok_or_else(|| unavailable(NO_USER_REASON))?;
        let target = pgpass::Target {
            host: &host,
            port,
            database: &database,
            user: &user,
        };
        let password = pgpass_or_azure_token(
            &target,
            is_azure_postgres_host(&host),
            lookup.pgpass_file,
            &lookup.sign_in,
        )?;
        (user, password)
    } else {
        let password = text(configuration, "password")
            .map(Secret::new)
            .or_else(|| saved.and_then(|saved| saved.password.clone()));
        let Some(password) = password else {
            return Err(unavailable(missing_password_reason(
                connection,
                saved.is_some(),
                credentials_problem,
            )));
        };
        (
            user.ok_or_else(|| unavailable(NO_USER_REASON))?,
            PasswordSource::Resolved(password),
        )
    };

    let tunnel = tunnel_handler
        .map(|handler| build_tunnel(id, handler, store, &host, port, &expand))
        .transpose()?;

    Ok(ConnectParams {
        host,
        port,
        database,
        user,
        sslmode: password.effective_sslmode(sslmode),
        password,
        tunnel,
    })
}

fn build_tunnel<E>(
    id: &str,
    handler: &Value,
    store: &CredentialStore,
    database_host: &str,
    database_port: u16,
    expand: &E,
) -> Result<TunnelParams, ConnectionStatus>
where
    E: Fn(&str) -> Result<String, ConnectionStatus>,
{
    let properties = handler.get("properties").unwrap_or(&NULL);
    let expanded = |key: &str| -> Result<Option<String>, ConnectionStatus> {
        text(properties, key)
            .map(|value| expand(&value))
            .transpose()
            .map(|value| value.filter(|value| !value.is_empty()))
    };
    let host = expanded("host")?.ok_or_else(|| unavailable("SSH tunnel has no host"))?;
    let port = match expanded("port")? {
        Some(raw) => {
            parse_port(&raw).ok_or_else(|| unavailable("SSH tunnel port is not a valid number"))?
        }
        None => 22,
    };
    let saved = store.ssh(id);
    let user = text(handler, "user")
        .or_else(|| saved.and_then(|saved| saved.user.clone()))
        .map(|value| expand(&value))
        .transpose()?
        .filter(|user| !user.is_empty())
        .ok_or_else(|| unavailable(SSH_USER_REASON))?;
    let password = text(handler, "password")
        .map(Secret::new)
        .or_else(|| saved.and_then(|saved| saved.password.clone()))
        .filter(|password| !password.expose().is_empty());
    let auth = match text(properties, "authType").as_deref() {
        Some("PUBLIC_KEY") => {
            let key_path = expanded("keyPath")?
                .map(PathBuf::from)
                .ok_or_else(|| unavailable("SSH key file not set in DBeaver"))?;
            check_key_file(&key_path)?;
            SshAuth::PublicKey {
                key_path,
                passphrase: password,
            }
        }
        None | Some("PASSWORD") => {
            SshAuth::Password(password.ok_or_else(|| unavailable(SSH_PASSWORD_REASON))?)
        }
        Some("AGENT") => SshAuth::Agent,
        Some(other) => {
            return Err(unsupported(format!(
                "SSH authentication type {other} is not supported"
            )));
        }
    };
    let (remote_host, remote_port) = match expanded("remoteHost")? {
        Some(remote_host) => {
            let remote_port = match expanded("remotePort")? {
                Some(raw) => parse_port(&raw)
                    .ok_or_else(|| unavailable("SSH tunnel remote port is not a valid number"))?,
                None => database_port,
            };
            (remote_host, remote_port)
        }
        None => (database_host.to_string(), database_port),
    };
    let host_key = if is_true(properties.get("bypassHostVerification")) {
        HostKeyPolicy::Bypass
    } else {
        HostKeyPolicy::Verify {
            known_hosts: Vec::new(),
            alias: None,
        }
    };
    Ok(TunnelParams {
        hops: vec![SshHop {
            label: host.clone(),
            host,
            port,
            user,
            auth,
            host_key,
        }],
        remote_host,
        remote_port,
    })
}

fn check_key_file(path: &Path) -> Result<(), ConnectionStatus> {
    match fs::metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Err(unavailable(format!(
            "SSH key file not found: {}",
            path.display()
        ))),
        Ok(metadata) if !metadata.is_file() => Err(unavailable(format!(
            "SSH key path is not a file: {}",
            path.display()
        ))),
        _ if fs::File::open(path).is_err() => Err(unavailable(format!(
            "SSH key file is not readable: {}",
            path.display()
        ))),
        _ => Ok(()),
    }
}

pub(super) fn parse_port(raw: &str) -> Option<u16> {
    let raw = raw.trim();
    let port = raw.parse::<u16>().ok().or_else(|| {
        let number = raw.parse::<f64>().ok()?;
        (number.fract() == 0.0 && (0.0..=f64::from(u16::MAX)).contains(&number))
            .then_some(number as u16)
    })?;
    (port != 0).then_some(port)
}

pub(super) fn is_true(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(text)) => text.trim().eq_ignore_ascii_case("true"),
        _ => false,
    }
}

fn jump_server_count(properties: &Value) -> f64 {
    match properties.get("jumpServer.count") {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(0.0),
        Some(Value::String(text)) => text.trim().parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn missing_password_reason(
    connection: &Value,
    has_entry: bool,
    credentials_problem: Option<&str>,
) -> String {
    let save_password = connection
        .get("save-password")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !save_password {
        return NO_PASSWORD_REASON.to_string();
    }
    if has_entry {
        return NO_PASSWORD_REASON.to_string();
    }
    match credentials_problem {
        Some(problem) => format!("{MASTER_PASSWORD_REASON} ({problem})"),
        None => MASTER_PASSWORD_REASON.to_string(),
    }
}

pub(super) fn find_tunnel_handler(
    configuration: &Value,
) -> Result<Option<&Value>, ConnectionStatus> {
    let Some(handlers) = configuration.get("handlers").and_then(Value::as_object) else {
        return Ok(None);
    };
    let mut tunnel = None;
    for (handler_id, handler) in handlers {
        if handler.get("enabled").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let kind = text(handler, "type").unwrap_or_default();
        let marker = format!("{handler_id} {kind}").to_ascii_lowercase();
        if marker.contains("proxy") || marker.contains("socks") {
            return Err(unsupported(PROXY_REASON));
        }
        if marker.contains("ssh") || marker.contains("tunnel") {
            let properties = handler.get("properties").unwrap_or(&NULL);
            if jump_server_count(properties) > 0.0 {
                return Err(unsupported(JUMP_SERVER_REASON));
            }
            tunnel.get_or_insert(handler);
        }
    }
    Ok(tunnel)
}

fn property_sslmode(properties: Option<&Value>) -> Option<String> {
    properties?.as_object()?.iter().find_map(|(key, value)| {
        let key = key.to_ascii_lowercase();
        (key == "sslmode" || key.ends_with("-sslmode@") || key.ends_with(".sslmode"))
            .then(|| value.as_str().map(str::to_string))
            .flatten()
    })
}

fn parse_sslmode(raw: Option<&str>) -> Result<SslMode, ConnectionStatus> {
    match raw.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        None | Some("") | Some("prefer") => Ok(SslMode::Prefer),
        Some("disable") => Ok(SslMode::Disable),
        Some("allow") => Ok(SslMode::Allow),
        Some("require") => Ok(SslMode::Require),
        Some("verify-ca") => Ok(SslMode::VerifyCa),
        Some("verify-full") => Ok(SslMode::VerifyFull),
        Some(other) => Err(unavailable(format!("unsupported sslmode '{other}'"))),
    }
}

struct UrlParts {
    host: String,
    port: Option<u16>,
    database: Option<String>,
    query: Vec<(String, String)>,
}

fn parse_jdbc_url(url: &str) -> Result<UrlParts, String> {
    let rest = url
        .strip_prefix("jdbc:postgresql://")
        .or_else(|| url.strip_prefix("postgresql://"))
        .ok_or_else(|| "connection url is not a PostgreSQL JDBC url".to_string())?;
    let (location, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, path) = location.split_once('/').unwrap_or((location, ""));
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let (host, port_text) = match authority.strip_prefix('[') {
        Some(bracketed) => {
            let (host, after) = bracketed
                .split_once(']')
                .ok_or_else(|| "connection url has an invalid host".to_string())?;
            (host, after.strip_prefix(':'))
        }
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    if host.is_empty() || host.contains(',') {
        return Err("connection url must name exactly one host".to_string());
    }
    let port = port_text
        .filter(|port| !port.is_empty())
        .map(|port| {
            port.parse::<u16>()
                .map_err(|_| "connection url port is not a valid number".to_string())
        })
        .transpose()?;
    let query = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    Ok(UrlParts {
        host: host.to_string(),
        port,
        database: (!path.is_empty()).then(|| path.to_string()),
        query,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    mod integration {
        use super::*;

        #[test]
        fn les_suffixes_de_fichiers_data_sources_sont_reconnus() {
            let dir = tempfile::TempDir::new().unwrap();
            for name in [
                "data-sources.json",
                "data-sources-2.json",
                "data-sources.json.bak",
                "data-sourcesx.json",
                "other.json",
            ] {
                fs::write(dir.path().join(name), "{}").unwrap();
            }
            let suffixes: Vec<String> = data_source_files(dir.path())
                .into_iter()
                .map(|(suffix, _)| suffix)
                .collect();
            assert_eq!(suffixes, vec!["".to_string(), "-2".to_string()]);
        }
    }

    mod unit {
        use super::*;

        #[test]
        fn une_url_ipv6_avec_requete_est_decomposee() {
            let parts = parse_jdbc_url("jdbc:postgresql://[::1]:5555/app?sslmode=disable").unwrap();
            assert_eq!(parts.host, "::1");
            assert_eq!(parts.port, Some(5555));
            assert_eq!(parts.database.as_deref(), Some("app"));
            assert_eq!(
                parts.query,
                vec![("sslmode".to_string(), "disable".to_string())]
            );
        }

        #[test]
        fn une_url_sans_port_ni_base_est_acceptee() {
            let parts = parse_jdbc_url("jdbc:postgresql://h.example/").unwrap();
            assert_eq!(parts.host, "h.example");
            assert_eq!(parts.port, None);
            assert_eq!(parts.database, None);
        }

        #[test]
        fn une_url_multi_hotes_est_rejetee() {
            assert!(parse_jdbc_url("jdbc:postgresql://a:1,b:2/db").is_err());
        }

        #[test]
        fn un_sslmode_inconnu_est_refuse() {
            assert!(parse_sslmode(Some("bogus")).is_err());
            assert_eq!(parse_sslmode(None).unwrap(), SslMode::Prefer);
        }
    }
}
