use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub dbeaver_home: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            dbeaver_home: std::env::var_os("PG_PSEUDO_DBEAVER_HOME").map(PathBuf::from),
            data_dir: std::env::var_os("PG_PSEUDO_DATA_DIR").map(PathBuf::from),
        }
    }
}
