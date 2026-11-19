use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::credentials::{CredentialStore, Secret};
use super::variables::{process_env, resolve};
use super::{ConnectParams, ConnectionEntry, ConnectionStatus, SslMode};

static NULL: Value = Value::Null;

const SSH_REASON: &str = "SSH tunnel not supported in this version";
const PROXY_REASON: &str = "proxy not supported in this version";
const MASTER_PASSWORD_REASON: &str = "credentials protected by DBeaver master password or secure storage; unlock or re-save them in DBeaver";
const NO_PASSWORD_REASON: &str = "no saved password for this connection in DBeaver";
const UNREADABLE_REASON: &str = "data-sources file is not valid JSON (the DBeaver project may be encrypted); its connections cannot be listed";

pub fn load(root: &Path) -> Vec<ConnectionEntry> {
    let mut entries = Vec::new();
    for (project, dir) in project_dirs(root) {
        for (suffix, path) in data_source_files(&dir) {
            entries.extend(load_file(&project, &dir, &suffix, &path));
        }
    }
    entries
}

fn project_dirs(root: &Path) -> Vec<(String, PathBuf)> {
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

fn data_source_files(dir: &Path) -> Vec<(String, PathBuf)> {
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

fn load_file(project: &str, dir: &Path, suffix: &str, path: &Path) -> Vec<ConnectionEntry> {
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
            )
        })
        .collect()
}

fn unreadable_entry(project: &str, file_label: &str) -> ConnectionEntry {
    let label = format!("{project}/{file_label}");
    ConnectionEntry {
        id: label.clone(),
        name: label,
        folder: None,
        project: project.to_string(),
        engine: "unknown".to_string(),
        connection_type: "unknown".to_string(),
        database: None,
        status: ConnectionStatus::Unavailable {
            reason: UNREADABLE_REASON.to_string(),
        },
        params: None,
    }
}

fn load_credentials(dir: &Path, suffix: &str) -> (CredentialStore, Option<String>) {
    let path = dir.join(format!("credentials-config{suffix}.json"));
    let Ok(bytes) = fs::read(&path) else {
        return (CredentialStore::default(), None);
    };
    match CredentialStore::from_encrypted(&bytes) {
        Ok(store) => (store, None),
        Err(error) => (CredentialStore::default(), Some(error.to_string())),
    }
}

fn text(value: &Value, key: &str) -> Option<String> {
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
    ) {
        Ok(params) => (ConnectionStatus::Available, Some(params)),
        Err(status) => (status, None),
    };
    let database = params
        .as_ref()
        .map(|params| params.database.clone())
        .or(database);
    ConnectionEntry {
        id: id.to_string(),
        name: text(connection, "name").unwrap_or_else(|| id.to_string()),
        folder: text(connection, "folder"),
        project: project.to_string(),
        engine,
        connection_type,
        database,
        status,
        params,
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

fn evaluate(
    id: &str,
    engine: &str,
    connection: &Value,
    configuration: &Value,
    store: &CredentialStore,
    credentials_problem: Option<&str>,
) -> Result<ConnectParams, ConnectionStatus> {
    if engine != "postgresql" {
        return Err(unsupported(format!(
            "engine {engine} is not supported, only PostgreSQL"
        )));
    }
    check_handlers(configuration)?;
    match text(configuration, "auth-model").as_deref() {
        None | Some("native") => {}
        Some(model) => {
            return Err(unsupported(format!(
                "authentication model {model} is not supported in this version"
            )));
        }
    }

    let own_field = |name: &str| match name {
        "host" | "port" | "database" | "user" => text(configuration, name),
        _ => None,
    };
    let expand = |input: &str| resolve(input, own_field, process_env).map_err(unavailable);

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

    let saved = store.get(id);
    let plain_user = text(configuration, "user")
        .map(|value| expand(&value))
        .transpose()?;
    let plain_password = text(configuration, "password");
    let user = plain_user.or_else(|| saved.and_then(|saved| saved.user.clone()));
    let password = plain_password
        .map(Secret::new)
        .or_else(|| saved.and_then(|saved| saved.password.clone()));

    let Some(password) = password else {
        return Err(unavailable(missing_password_reason(
            connection,
            saved.is_some(),
            credentials_problem,
        )));
    };
    let user = user
        .filter(|user| !user.is_empty())
        .ok_or_else(|| unavailable("no saved user name for this connection in DBeaver"))?;

    Ok(ConnectParams {
        host,
        port: port.unwrap_or(5432),
        database: database.unwrap_or_else(|| "postgres".to_string()),
        user,
        password,
        sslmode,
    })
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

fn check_handlers(configuration: &Value) -> Result<(), ConnectionStatus> {
    let Some(handlers) = configuration.get("handlers").and_then(Value::as_object) else {
        return Ok(());
    };
    for (handler_id, handler) in handlers {
        if handler.get("enabled").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let kind = text(handler, "type").unwrap_or_default();
        let marker = format!("{handler_id} {kind}").to_ascii_lowercase();
        if marker.contains("ssh") || marker.contains("tunnel") {
            return Err(unsupported(SSH_REASON));
        }
        if marker.contains("proxy") || marker.contains("socks") {
            return Err(unsupported(PROXY_REASON));
        }
    }
    Ok(())
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

    #[test]
    fn parses_url_with_ipv6_and_query() {
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
    fn url_without_port_or_database() {
        let parts = parse_jdbc_url("jdbc:postgresql://h.example/").unwrap();
        assert_eq!(parts.host, "h.example");
        assert_eq!(parts.port, None);
        assert_eq!(parts.database, None);
    }

    #[test]
    fn multi_host_url_is_rejected() {
        assert!(parse_jdbc_url("jdbc:postgresql://a:1,b:2/db").is_err());
    }

    #[test]
    fn unknown_sslmode_is_refused() {
        assert!(parse_sslmode(Some("bogus")).is_err());
        assert_eq!(parse_sslmode(None).unwrap(), SslMode::Prefer);
    }

    #[test]
    fn data_source_suffixes_are_matched() {
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
