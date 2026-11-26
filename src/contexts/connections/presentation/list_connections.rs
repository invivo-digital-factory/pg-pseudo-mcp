use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Map, Value, json};

use crate::common::redact_home;
use crate::contexts::connections::domain::types::{ConnectionEntry, ConnectionStatus};
use crate::contexts::connections::use_cases::ListConnections;

const COLUMN_POLICY_IGNORED: &str = "column_policy_ignored";

pub async fn list_connections(use_case: &ListConnections) -> CallToolResult {
    list_connections_with_warnings(use_case, &[]).await
}

pub async fn list_connections_with_warnings(
    use_case: &ListConnections,
    column_policy_warnings: &[String],
) -> CallToolResult {
    let discovery = match use_case.execute().await {
        Ok(discovery) => discovery,
        Err(error) => {
            return CallToolResult::error(vec![ContentBlock::text(
                json!({ "error": error.to_string() }).to_string(),
            )]);
        }
    };
    let connections: Vec<Value> = discovery.connections.iter().map(describe).collect();
    let mut body = Map::new();
    body.insert("connections".to_string(), Value::Array(connections));
    if let Some(hint) = discovery.hint {
        body.insert("hint".to_string(), Value::String(redact_home(&hint)));
    }
    if !column_policy_warnings.is_empty() {
        let warnings = column_policy_warnings
            .iter()
            .map(
                |message| json!({ "code": COLUMN_POLICY_IGNORED, "message": redact_home(message) }),
            )
            .collect();
        body.insert("warnings".to_string(), Value::Array(warnings));
    }
    CallToolResult::success(vec![ContentBlock::text(Value::Object(body).to_string())])
}

fn describe(entry: &ConnectionEntry) -> Value {
    let mut item = Map::new();
    item.insert("name".to_string(), json!(entry.name));
    item.insert("engine".to_string(), json!(entry.engine));
    item.insert("source".to_string(), json!(entry.source.as_str()));
    match &entry.status {
        ConnectionStatus::Available => {
            item.insert("status".to_string(), json!("available"));
        }
        ConnectionStatus::Unavailable { reason } => {
            item.insert("status".to_string(), json!("unavailable"));
            item.insert("reason".to_string(), json!(redact_home(reason)));
        }
        ConnectionStatus::Unsupported { reason } => {
            item.insert("status".to_string(), json!("unsupported"));
            item.insert("reason".to_string(), json!(redact_home(reason)));
        }
    }
    if let Some(database) = &entry.database {
        item.insert("database".to_string(), json!(database));
    }
    let tunnel_hops = entry
        .params
        .as_ref()
        .and_then(|params| params.tunnel.as_ref())
        .map(|tunnel| tunnel.hops.as_slice());
    let mongo_hops = entry
        .mongo
        .as_ref()
        .and_then(|mongo| mongo.ssh_hops.as_deref());
    let bypassed = tunnel_hops
        .into_iter()
        .chain(mongo_hops)
        .flatten()
        .any(|hop| !hop.host_key.is_verified());
    if bypassed {
        item.insert("ssh_host_key_check".to_string(), json!("bypassed"));
    }
    item.insert("type".to_string(), json!(entry.connection_type));
    if let Some(folder) = &entry.folder {
        item.insert("folder".to_string(), json!(folder));
    }
    Value::Object(item)
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::connections::domain::types::{
        ConnectionSource, HostKeyPolicy, MongoConnectParams, MongoReadPreference, MongoTransport,
        SshAuth, SshHop,
    };

    fn mongo_entry(host_key: HostKeyPolicy) -> ConnectionEntry {
        ConnectionEntry {
            id: "atlas".to_string(),
            name: "atlas".to_string(),
            declared_name: "atlas".to_string(),
            source: ConnectionSource::File,
            folder: None,
            project: "file".to_string(),
            engine: "mongodb".to_string(),
            connection_type: "dev".to_string(),
            database: None,
            status: ConnectionStatus::Available,
            params: None,
            mongo: Some(MongoConnectParams {
                read_preference: MongoReadPreference::default(),
                transport: MongoTransport::Srv {
                    host: "pl-0.atlas.scarif.test".to_string(),
                },
                replica_set: None,
                database: None,
                user: "reader".to_string(),
                password_command: vec!["/bin/echo".to_string()],
                ssh_hops: Some(vec![SshHop {
                    label: "bastion".to_string(),
                    host: "bastion.scarif.test".to_string(),
                    port: 22,
                    user: "jump".to_string(),
                    auth: SshAuth::Agent,
                    host_key,
                }]),
            }),
        }
    }

    #[test]
    fn une_connexion_mongodb_dont_un_saut_ne_verifie_pas_la_cle_est_listee_avec_le_drapeau() {
        let listed = describe(&mongo_entry(HostKeyPolicy::Bypass));
        assert_eq!(listed["ssh_host_key_check"], "bypassed");
    }

    #[test]
    fn une_connexion_mongodb_dont_les_sauts_verifient_la_cle_est_listee_sans_le_drapeau() {
        let listed = describe(&mongo_entry(HostKeyPolicy::Verify {
            known_hosts: Vec::new(),
            alias: None,
        }));
        assert!(listed.get("ssh_host_key_check").is_none(), "{listed}");
    }
}
