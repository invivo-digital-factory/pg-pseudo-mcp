use std::path::{Path, PathBuf};

use super::workspace;
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::services::disambiguate;
use crate::contexts::connections::domain::types::Discovery;

pub fn default_dbeaver_home() -> Option<PathBuf> {
    let dirs = directories::BaseDirs::new()?;
    if cfg!(target_os = "macos") {
        Some(dirs.home_dir().join("Library").join("DBeaverData"))
    } else {
        Some(dirs.data_dir().join("DBeaverData"))
    }
}

pub fn discover(
    dbeaver_home: Option<&Path>,
    tokens: &dyn AzureSignInTokenProvider,
    pgpass_file: Option<&Path>,
) -> Discovery {
    discover_in(dbeaver_home, tokens, pgpass_file, None)
}

pub fn discover_in(
    dbeaver_home: Option<&Path>,
    tokens: &dyn AzureSignInTokenProvider,
    pgpass_file: Option<&Path>,
    database: Option<&str>,
) -> Discovery {
    let Some(root) = dbeaver_home
        .map(Path::to_path_buf)
        .or_else(default_dbeaver_home)
    else {
        return Discovery {
            connections: Vec::new(),
            hint: Some(
                "DBeaver data directory could not be located; set SCARIF_DBEAVER_HOME".to_string(),
            ),
        };
    };
    if !root.join("workspace6").is_dir() {
        return Discovery {
            connections: Vec::new(),
            hint: Some(format!(
                "no DBeaver workspace found at {}; set SCARIF_DBEAVER_HOME to the DBeaverData directory",
                root.display()
            )),
        };
    }
    let mut connections = workspace::load(&root, tokens, pgpass_file, database);
    disambiguate(&mut connections);
    connections.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let hint = connections
        .is_empty()
        .then(|| "the DBeaver workspace contains no connections".to_string());
    Discovery { connections, hint }
}
