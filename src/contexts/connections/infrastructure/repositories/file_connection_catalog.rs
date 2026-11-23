use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::types::Discovery;
use crate::contexts::connections::infrastructure::data_sources::connection_file::{
    DbeaverAccess, FileEvaluator, Location, locate_from_environment, parse_file,
};
use crate::contexts::connections::infrastructure::data_sources::openssh::OpenSshResolver;

pub struct FileLoad {
    pub discovery: Discovery,
    pub dbeaver: DbeaverAccess,
    pub empty_hint: String,
    pub path: Option<PathBuf>,
}

pub struct FileConnectionCatalog {
    configured: Option<PathBuf>,
    tokens: Arc<dyn AzureSignInTokenProvider>,
    resolver: Arc<OpenSshResolver>,
}

impl FileConnectionCatalog {
    pub fn new(
        configured: Option<PathBuf>,
        tokens: Arc<dyn AzureSignInTokenProvider>,
        resolver: Arc<OpenSshResolver>,
    ) -> Self {
        Self {
            configured,
            tokens,
            resolver,
        }
    }

    pub fn load(&self, database: Option<&str>) -> FileLoad {
        let location = locate_from_environment(self.configured.as_deref());
        let empty_hint = empty_hint(location.as_ref());
        let mut load = FileLoad {
            discovery: Discovery {
                connections: Vec::new(),
                hint: None,
            },
            dbeaver: DbeaverAccess::Read,
            empty_hint,
            path: location.as_ref().map(|location| location.path.clone()),
        };
        let Some(location) = location else {
            return load;
        };
        let contents = match std::fs::read_to_string(&location.path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound && !location.explicit => {
                return load;
            }
            Err(error) => {
                load.dbeaver = if error.kind() == ErrorKind::NotFound {
                    DbeaverAccess::Read
                } else {
                    DbeaverAccess::Blocked
                };
                load.discovery.hint = Some(if error.kind() == ErrorKind::NotFound {
                    format!(
                        "PG_PSEUDO_CONNECTIONS_FILE points at {}, which does not exist; create it or run `pg-pseudo-mcp connections import-dbeaver` to write it from DBeaver",
                        location.path.display()
                    )
                } else {
                    format!(
                        "the connection file {} cannot be read",
                        location.path.display()
                    )
                });
                return load;
            }
        };
        let parsed = match parse_file(&contents) {
            Ok(parsed) => parsed,
            Err(error) => {
                load.dbeaver = DbeaverAccess::Blocked;
                load.discovery.hint = Some(format!(
                    "the connection file {} cannot be parsed (line {}, column {}); fix its syntax; DBeaver connections are unavailable until then",
                    location.path.display(),
                    error.line,
                    error.column
                ));
                return load;
            }
        };
        let evaluator = FileEvaluator::new(self.tokens.as_ref(), &self.resolver, database);
        load.discovery.connections = parsed
            .entries
            .iter()
            .map(|entry| evaluator.entry(entry))
            .collect();
        load.dbeaver = parsed.dbeaver;
        if !parsed.notes.is_empty() {
            load.discovery.hint = Some(format!(
                "the connection file {} has problems: {}",
                location.path.display(),
                parsed.notes.join("; ")
            ));
        }
        load
    }
}

fn empty_hint(location: Option<&Location>) -> String {
    let target = location.map_or_else(
        || "the connection file".to_string(),
        |location| location.path.display().to_string(),
    );
    format!(
        "no connection found: create {target} or run `pg-pseudo-mcp connections import-dbeaver` to write it from DBeaver"
    )
}
