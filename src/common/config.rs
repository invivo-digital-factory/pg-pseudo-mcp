use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub dbeaver_home: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub ssh_known_hosts: Option<PathBuf>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self::from_vars(
            std::env::var_os("PG_PSEUDO_DBEAVER_HOME"),
            std::env::var_os("PG_PSEUDO_DATA_DIR"),
            std::env::var_os("PG_PSEUDO_SSH_KNOWN_HOSTS"),
        )
    }

    pub(crate) fn from_vars(
        dbeaver_home: Option<OsString>,
        data_dir: Option<OsString>,
        ssh_known_hosts: Option<OsString>,
    ) -> Self {
        let absolute =
            |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        Self {
            dbeaver_home: absolute(dbeaver_home),
            data_dir: absolute(data_dir),
            ssh_known_hosts: absolute(ssh_known_hosts),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn empty_or_relative_directories_fall_back_to_defaults() {
        let config = AppConfig::from_vars(
            Some(OsString::new()),
            Some(OsString::from("relative")),
            Some(OsString::from("known_hosts")),
        );
        assert_eq!(config.dbeaver_home, None);
        assert_eq!(config.data_dir, None);
        assert_eq!(config.ssh_known_hosts, None);
        let config = AppConfig::from_vars(
            None,
            Some(OsString::from("/tmp/pg-pseudo")),
            Some(OsString::from("/tmp/known_hosts")),
        );
        assert_eq!(config.data_dir, Some(PathBuf::from("/tmp/pg-pseudo")));
        assert_eq!(
            config.ssh_known_hosts,
            Some(PathBuf::from("/tmp/known_hosts"))
        );
    }
}
