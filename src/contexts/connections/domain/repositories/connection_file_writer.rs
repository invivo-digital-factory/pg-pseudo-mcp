use std::path::PathBuf;

use crate::contexts::connections::domain::errors::ConnectionFileWriteError;
use crate::contexts::connections::domain::types::ConnectionDefinition;

pub trait ConnectionFileWriter: Send + Sync {
    fn path(&self) -> Result<PathBuf, ConnectionFileWriteError>;

    fn write(
        &self,
        definitions: &[ConnectionDefinition],
        overwrite: bool,
    ) -> Result<PathBuf, ConnectionFileWriteError>;
}
