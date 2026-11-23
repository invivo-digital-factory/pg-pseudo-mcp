use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::common::redact_home;
use crate::contexts::connections::domain::repositories::ColumnPolicyFiles;
use crate::contexts::connections::domain::types::ColumnPolicyFile;
use crate::contexts::connections::infrastructure::data_sources::column_policy_file::parse_policy;

pub struct TomlColumnPolicyFiles {
    shared: Option<PathBuf>,
    local: Option<PathBuf>,
}

impl TomlColumnPolicyFiles {
    pub fn new(shared: Option<PathBuf>, local: Option<PathBuf>) -> Self {
        Self { shared, local }
    }
}

impl ColumnPolicyFiles for TomlColumnPolicyFiles {
    fn read(&self) -> Vec<ColumnPolicyFile> {
        let shared = self.shared.as_deref().map(|path| read_file(path, true));
        let local = self.local.as_deref().map(|path| read_file(path, false));
        shared.into_iter().chain(local).flatten().collect()
    }
}

fn read_file(path: &Path, required: bool) -> Option<ColumnPolicyFile> {
    let content = match std::fs::read_to_string(path) {
        Ok(contents) => parse_policy(&contents),
        Err(error) if error.kind() == ErrorKind::NotFound && !required => return None,
        Err(error) if error.kind() == ErrorKind::NotFound => Err("the file does not exist".into()),
        Err(_) => Err("the file cannot be read".into()),
    };
    Some(ColumnPolicyFile {
        file: redact_home(&path.display().to_string()),
        content,
    })
}
