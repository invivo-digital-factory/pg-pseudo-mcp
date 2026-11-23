use std::sync::Arc;

use crate::contexts::connections::domain::repositories::ColumnPolicyFiles;
use crate::contexts::connections::domain::types::ColumnPolicyFile;

pub struct ReadColumnPolicyFiles {
    files: Arc<dyn ColumnPolicyFiles>,
}

impl ReadColumnPolicyFiles {
    pub fn new(files: Arc<dyn ColumnPolicyFiles>) -> Self {
        Self { files }
    }

    pub fn execute(&self) -> Vec<ColumnPolicyFile> {
        self.files.read()
    }
}
