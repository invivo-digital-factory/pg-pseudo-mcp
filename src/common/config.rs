use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub dbeaver_home: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub ssh_known_hosts: Option<PathBuf>,
    pub ssh_config: Option<PathBuf>,
    pub connections_file: Option<PathBuf>,
    pub columns_file: Option<PathBuf>,
    pub xdg_config_home: Option<OsString>,
    pub home_dir: Option<PathBuf>,
    pub data_home: Option<PathBuf>,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let (config, deprecations) = Self::from_lookup(|name| std::env::var_os(name));
        for deprecation in deprecations {
            eprintln!("{deprecation}");
        }
        let base_dirs = directories::BaseDirs::new();
        config
            .with_user_config_dirs(
                std::env::var_os("XDG_CONFIG_HOME"),
                base_dirs.as_ref().map(|dirs| dirs.home_dir().to_path_buf()),
            )
            .with_data_home(base_dirs.map(|dirs| dirs.data_dir().to_path_buf()))
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<OsString>) -> (Self, Vec<String>) {
        let mut deprecations = Vec::new();
        let mut read = |name: &str| {
            let current = format!("SCARIF_{name}");
            get(&current).or_else(|| {
                let legacy = format!("PG_PSEUDO_{name}");
                let value = get(&legacy)?;
                deprecations.push(format!(
                    "warning: {legacy} is deprecated and will be removed in a future version; set {current} instead"
                ));
                Some(value)
            })
        };
        let config = Self::from_vars(
            read("DBEAVER_HOME"),
            read("DATA_DIR"),
            read("SSH_KNOWN_HOSTS"),
            read("SSH_CONFIG"),
            read("CONNECTIONS_FILE"),
            read("COLUMNS_FILE"),
        );
        (config, deprecations)
    }

    pub fn with_user_config_dirs(
        mut self,
        xdg_config_home: Option<OsString>,
        home_dir: Option<PathBuf>,
    ) -> Self {
        self.xdg_config_home = xdg_config_home;
        self.home_dir = home_dir;
        self
    }

    pub fn with_data_home(mut self, data_home: Option<PathBuf>) -> Self {
        self.data_home = data_home;
        self
    }

    pub(crate) fn from_vars(
        dbeaver_home: Option<OsString>,
        data_dir: Option<OsString>,
        ssh_known_hosts: Option<OsString>,
        ssh_config: Option<OsString>,
        connections_file: Option<OsString>,
        columns_file: Option<OsString>,
    ) -> Self {
        let absolute =
            |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        Self {
            dbeaver_home: absolute(dbeaver_home),
            data_dir: absolute(data_dir),
            ssh_known_hosts: absolute(ssh_known_hosts),
            ssh_config: absolute(ssh_config),
            connections_file: absolute(connections_file),
            columns_file: absolute(columns_file),
            xdg_config_home: None,
            home_dir: None,
            data_home: None,
        }
    }
}

#[cfg(test)]
mod unit {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::*;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let values: HashMap<String, OsString> = pairs
            .iter()
            .map(|(name, value)| (name.to_string(), OsString::from(value)))
            .collect();
        move |name: &str| values.get(name).cloned()
    }

    #[test]
    fn la_nouvelle_variable_l_emporte_sur_l_ancienne_sans_avertissement() {
        let (config, deprecations) = AppConfig::from_lookup(lookup(&[
            ("SCARIF_DATA_DIR", "/new"),
            ("PG_PSEUDO_DATA_DIR", "/old"),
        ]));
        assert_eq!(config.data_dir, Some(PathBuf::from("/new")));
        assert!(deprecations.is_empty(), "{deprecations:?}");
    }

    #[test]
    fn l_ancienne_variable_seule_fonctionne_et_avertit_sans_valeur() {
        let (config, deprecations) = AppConfig::from_lookup(lookup(&[
            ("PG_PSEUDO_DATA_DIR", "/old-secret-looking-path"),
            ("PG_PSEUDO_SSH_CONFIG", "/old/ssh_config"),
        ]));
        assert_eq!(
            config.data_dir,
            Some(PathBuf::from("/old-secret-looking-path"))
        );
        assert_eq!(config.ssh_config, Some(PathBuf::from("/old/ssh_config")));
        assert_eq!(deprecations.len(), 2, "{deprecations:?}");
        assert!(
            deprecations[0].contains("PG_PSEUDO_DATA_DIR"),
            "{deprecations:?}"
        );
        assert!(
            deprecations[0].contains("SCARIF_DATA_DIR"),
            "{deprecations:?}"
        );
        assert!(
            deprecations.iter().all(|line| !line.contains("/old")),
            "{deprecations:?}"
        );
    }

    #[test]
    fn chaque_ancienne_variable_a_son_remplacant() {
        for name in [
            "DBEAVER_HOME",
            "DATA_DIR",
            "SSH_KNOWN_HOSTS",
            "SSH_CONFIG",
            "CONNECTIONS_FILE",
            "COLUMNS_FILE",
        ] {
            let old = format!("PG_PSEUDO_{name}");
            let (_, deprecations) = AppConfig::from_lookup(lookup(&[(old.as_str(), "/x")]));
            assert_eq!(deprecations.len(), 1, "{name}");
            assert!(
                deprecations[0].contains(&format!("SCARIF_{name}")),
                "{name}"
            );
        }
    }

    #[test]
    fn un_repertoire_vide_ou_relatif_retombe_sur_la_valeur_par_defaut() {
        let config = AppConfig::from_vars(
            Some(OsString::new()),
            Some(OsString::from("relative")),
            Some(OsString::from("known_hosts")),
            Some(OsString::from("ssh_config")),
            Some(OsString::from("connections.toml")),
            Some(OsString::from("columns.toml")),
        );
        assert_eq!(config.dbeaver_home, None);
        assert_eq!(config.data_dir, None);
        assert_eq!(config.ssh_known_hosts, None);
        assert_eq!(config.ssh_config, None);
        assert_eq!(config.connections_file, None);
        assert_eq!(config.columns_file, None);
        let config = AppConfig::from_vars(
            None,
            Some(OsString::from("/tmp/scarif")),
            Some(OsString::from("/tmp/known_hosts")),
            Some(OsString::from("/tmp/ssh_config")),
            Some(OsString::from("/tmp/connections.toml")),
            Some(OsString::from("/team/columns.toml")),
        );
        assert_eq!(
            config.columns_file,
            Some(PathBuf::from("/team/columns.toml"))
        );
        assert_eq!(config.data_dir, Some(PathBuf::from("/tmp/scarif")));
        assert_eq!(
            config.ssh_known_hosts,
            Some(PathBuf::from("/tmp/known_hosts"))
        );
        assert_eq!(config.ssh_config, Some(PathBuf::from("/tmp/ssh_config")));
        assert_eq!(
            config.connections_file,
            Some(PathBuf::from("/tmp/connections.toml"))
        );
    }
}
