use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Map, Value, json};

use crate::common::redact_home;
use crate::contexts::connections::domain::types::{ConnectionEntry, ConnectionStatus};
use crate::contexts::connections::use_cases::ListConnections;

pub async fn list_connections(use_case: &ListConnections) -> CallToolResult {
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
        body.insert("hint".to_string(), Value::String(hint));
    }
    CallToolResult::success(vec![ContentBlock::text(Value::Object(body).to_string())])
}

fn describe(entry: &ConnectionEntry) -> Value {
    let mut item = Map::new();
    item.insert("name".to_string(), json!(entry.name));
    item.insert("engine".to_string(), json!(entry.engine));
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
    let bypassed = entry
        .params
        .as_ref()
        .and_then(|params| params.tunnel.as_ref())
        .is_some_and(|tunnel| !tunnel.verify_host_key);
    if bypassed {
        item.insert("ssh_host_key_check".to_string(), json!("bypassed"));
    }
    item.insert("type".to_string(), json!(entry.connection_type));
    if let Some(folder) = &entry.folder {
        item.insert("folder".to_string(), json!(folder));
    }
    Value::Object(item)
}
