use rmcp::model::CallToolResult;
use serde_json::{Map, Value, json};

use crate::dbeaver::{ConnectionEntry, ConnectionStatus, discover};
use crate::state::AppConfig;
use crate::tools::result::json_result;

pub async fn list_connections(config: &AppConfig) -> CallToolResult {
    let discovery = discover(config.dbeaver_home.as_deref());
    let connections: Vec<Value> = discovery.connections.iter().map(describe).collect();
    let mut body = Map::new();
    body.insert("connections".to_string(), Value::Array(connections));
    if let Some(hint) = discovery.hint {
        body.insert("hint".to_string(), Value::String(hint));
    }
    json_result(&Value::Object(body))
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
            item.insert(
                "reason".to_string(),
                json!(crate::tools::errors::redact_home(reason)),
            );
        }
        ConnectionStatus::Unsupported { reason } => {
            item.insert("status".to_string(), json!("unsupported"));
            item.insert(
                "reason".to_string(),
                json!(crate::tools::errors::redact_home(reason)),
            );
        }
    }
    if let Some(database) = &entry.database {
        item.insert("database".to_string(), json!(database));
    }
    item.insert("type".to_string(), json!(entry.connection_type));
    if let Some(folder) = &entry.folder {
        item.insert("folder".to_string(), json!(folder));
    }
    Value::Object(item)
}
