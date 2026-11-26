use crate::contexts::connections::domain::types::HeavyQueryLimits;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::credentials::CredentialStore;
use super::discovery::default_dbeaver_home;
use super::workspace::{
    NULL, UNREADABLE_REASON, data_source_files, endpoint, expander, find_tunnel_handler,
    load_credentials, parse_port, pgpass_auth_model, project_dirs, text,
};
use crate::contexts::connections::domain::repositories::DbeaverDefinitions;
use crate::contexts::connections::domain::services::disambiguate;
use crate::contexts::connections::domain::types::{
    ConnectionEntry, ConnectionSource, ConnectionStatus, DbeaverDefinition, DbeaverRead,
    DbeaverTunnel, SkippedConnection,
};

const NO_USER_REASON: &str = "no saved user name for this connection in DBeaver";
const NO_SSH_USER_REASON: &str = "SSH user not saved in DBeaver";
const NO_SSH_HOST_REASON: &str = "SSH tunnel has no host";
const BAD_SSH_PORT_REASON: &str = "SSH tunnel port is not a valid number";

pub struct DbeaverWorkspaceDefinitions {
    dbeaver_home: Option<PathBuf>,
}

impl DbeaverWorkspaceDefinitions {
    pub fn new(dbeaver_home: Option<PathBuf>) -> Self {
        Self { dbeaver_home }
    }
}

impl DbeaverDefinitions for DbeaverWorkspaceDefinitions {
    fn read(&self) -> Result<DbeaverRead, String> {
        let root = self
            .dbeaver_home
            .clone()
            .or_else(default_dbeaver_home)
            .ok_or_else(|| {
                "DBeaver data directory could not be located; set SCARIF_DBEAVER_HOME".to_string()
            })?;
        if !root.join("workspace6").is_dir() {
            return Err(format!(
                "no DBeaver workspace found at {}; set SCARIF_DBEAVER_HOME to the DBeaverData directory",
                root.display()
            ));
        }
        Ok(read_definitions(&root))
    }
}

fn read_definitions(root: &Path) -> DbeaverRead {
    let mut stubs: Vec<ConnectionEntry> = Vec::new();
    let mut outcomes: Vec<Result<DbeaverDefinition, String>> = Vec::new();
    for (project, dir) in project_dirs(root) {
        for (suffix, path) in data_source_files(&dir) {
            let parsed = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .filter(Value::is_object);
            let Some(file) = parsed else {
                let label = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                stubs.push(stub(&project, &format!("{project}/{label}"), None));
                outcomes.push(Err(UNREADABLE_REASON.to_string()));
                continue;
            };
            let (store, _) = load_credentials(&dir, &suffix);
            let Some(connections) = file.get("connections").and_then(Value::as_object) else {
                continue;
            };
            for (id, connection) in connections {
                let name = text(connection, "name").unwrap_or_else(|| id.clone());
                stubs.push(stub(&project, id, Some(name)));
                outcomes.push(definition_of(id, connection, &store));
            }
        }
    }
    let mut pairs: Vec<(ConnectionEntry, Result<DbeaverDefinition, String>)> =
        stubs.into_iter().zip(outcomes).collect();
    let mut entries: Vec<ConnectionEntry> = pairs.iter().map(|(stub, _)| stub.clone()).collect();
    disambiguate(&mut entries);
    for ((stub, _), named) in pairs.iter_mut().zip(entries) {
        stub.name = named.name;
    }
    pairs.sort_by(|(a, _), (b, _)| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let mut read = DbeaverRead {
        definitions: Vec::new(),
        skipped: Vec::new(),
    };
    for (stub, outcome) in pairs {
        match outcome {
            Ok(definition) => read.definitions.push(DbeaverDefinition {
                name: stub.name,
                ..definition
            }),
            Err(reason) => read.skipped.push(SkippedConnection {
                name: stub.name,
                reason,
            }),
        }
    }
    read
}

fn stub(project: &str, id: &str, name: Option<String>) -> ConnectionEntry {
    let name = name.unwrap_or_else(|| id.to_string());
    ConnectionEntry {
        id: id.to_string(),
        declared_name: name.clone(),
        name,
        source: ConnectionSource::Dbeaver,
        folder: None,
        project: project.to_string(),
        engine: "unknown".to_string(),
        connection_type: "unknown".to_string(),
        heavy_query: HeavyQueryLimits::default(),
        database: None,
        status: ConnectionStatus::Available,
        params: None,
        mongo: None,
    }
}

fn reason_of(status: ConnectionStatus) -> String {
    match status {
        ConnectionStatus::Unavailable { reason } | ConnectionStatus::Unsupported { reason } => {
            reason
        }
        ConnectionStatus::Available => String::new(),
    }
}

fn definition_of(
    id: &str,
    connection: &Value,
    store: &CredentialStore,
) -> Result<DbeaverDefinition, String> {
    let engine = text(connection, "provider").unwrap_or_else(|| "unknown".to_string());
    if engine != "postgresql" {
        return Err(format!("engine {engine} is not supported, only PostgreSQL"));
    }
    let configuration = connection.get("configuration").unwrap_or(&NULL);
    let handler = find_tunnel_handler(configuration).map_err(reason_of)?;
    let pgpass_auth = pgpass_auth_model(configuration).map_err(reason_of)?;
    let expand = expander(configuration);
    let endpoint = endpoint(configuration, &expand).map_err(reason_of)?;
    let user = text(configuration, "user")
        .map(|value| expand(&value))
        .transpose()
        .map_err(reason_of)?
        .or_else(|| store.get(id).and_then(|saved| saved.user.clone()))
        .filter(|user| !user.is_empty())
        .ok_or_else(|| NO_USER_REASON.to_string())?;
    let mut host = endpoint.host;
    let mut port = endpoint.port;
    let tunnel = match handler {
        Some(handler) => {
            let (tunnel, remote) = tunnel_of(id, handler, store, &expand)?;
            if let Some((remote_host, remote_port)) = remote {
                host = remote_host;
                port = remote_port.unwrap_or(port);
            }
            Some(tunnel)
        }
        None => None,
    };
    Ok(DbeaverDefinition {
        name: String::new(),
        host,
        port,
        database: endpoint.database.unwrap_or_else(|| "postgres".to_string()),
        user,
        sslmode: endpoint.sslmode,
        connection_type: text(configuration, "type").unwrap_or_else(|| "dev".to_string()),
        pgpass_auth,
        tunnel,
    })
}

type RemoteEndpoint = Option<(String, Option<u16>)>;

fn tunnel_of<E>(
    id: &str,
    handler: &Value,
    store: &CredentialStore,
    expand: &E,
) -> Result<(DbeaverTunnel, RemoteEndpoint), String>
where
    E: Fn(&str) -> Result<String, ConnectionStatus>,
{
    let properties = handler.get("properties").unwrap_or(&NULL);
    let expanded = |key: &str| -> Result<Option<String>, String> {
        text(properties, key)
            .map(|value| expand(&value))
            .transpose()
            .map(|value| value.filter(|value| !value.is_empty()))
            .map_err(reason_of)
    };
    let host = expanded("host")?.ok_or_else(|| NO_SSH_HOST_REASON.to_string())?;
    let port = match expanded("port")? {
        Some(raw) => parse_port(&raw).ok_or_else(|| BAD_SSH_PORT_REASON.to_string())?,
        None => 22,
    };
    let user = text(handler, "user")
        .or_else(|| store.ssh(id).and_then(|saved| saved.user.clone()))
        .map(|value| expand(&value))
        .transpose()
        .map_err(reason_of)?
        .filter(|user| !user.is_empty())
        .ok_or_else(|| NO_SSH_USER_REASON.to_string())?;
    let key_path = (text(properties, "authType").as_deref() == Some("PUBLIC_KEY"))
        .then(|| expanded("keyPath"))
        .transpose()?
        .flatten()
        .map(PathBuf::from);
    let remote = match expanded("remoteHost")? {
        Some(remote_host) => {
            let remote_port = expanded("remotePort")?.and_then(|raw| parse_port(&raw));
            Some((remote_host, remote_port))
        }
        None => None,
    };
    Ok((
        DbeaverTunnel {
            host,
            port,
            user,
            key_path,
        },
        remote,
    ))
}
