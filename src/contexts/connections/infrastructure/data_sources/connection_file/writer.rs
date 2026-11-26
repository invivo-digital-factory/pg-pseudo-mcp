use std::io::Write;
use std::path::PathBuf;

use super::location::locate_for_write;
use crate::contexts::connections::domain::errors::ConnectionFileWriteError;
use crate::contexts::connections::domain::repositories::ConnectionFileWriter;
use crate::contexts::connections::domain::types::{
    AuthMode, ConnectionDefinition, EngineSettings, MongoTransport,
};

pub struct TomlConnectionFileWriter {
    configured: Option<PathBuf>,
}

impl TomlConnectionFileWriter {
    pub fn new(configured: Option<PathBuf>) -> Self {
        Self { configured }
    }
}

impl ConnectionFileWriter for TomlConnectionFileWriter {
    fn path(&self) -> Result<PathBuf, ConnectionFileWriteError> {
        let home = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
        locate_for_write(
            self.configured.as_deref(),
            std::env::var_os("XDG_CONFIG_HOME"),
            home.as_deref(),
        )
        .ok_or(ConnectionFileWriteError::NoLocation)
    }

    fn write(
        &self,
        definitions: &[ConnectionDefinition],
        overwrite: bool,
    ) -> Result<PathBuf, ConnectionFileWriteError> {
        let path = self.path()?;
        let io_error = |error: std::io::Error| ConnectionFileWriteError::Io {
            path: path.clone(),
            reason: error.to_string(),
        };
        if !overwrite && path.exists() {
            return Err(ConnectionFileWriteError::AlreadyExists(path));
        }
        let directory = path.parent().ok_or(ConnectionFileWriteError::NoLocation)?;
        std::fs::create_dir_all(directory).map_err(io_error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(io_error)?;
        temporary
            .write_all(render(definitions).as_bytes())
            .map_err(io_error)?;
        let placed = if overwrite {
            temporary.persist(&path)
        } else {
            temporary.persist_noclobber(&path)
        };
        if let Err(error) = placed {
            return Err(if error.error.kind() == std::io::ErrorKind::AlreadyExists {
                ConnectionFileWriteError::AlreadyExists(path.clone())
            } else {
                io_error(error.error)
            });
        }
        Ok(path)
    }
}

fn quoted(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

fn identity_lines(definition: &ConnectionDefinition) -> String {
    let name = quoted(&definition.name);
    let user = quoted(&definition.user);
    let kind = quoted(definition.connection_type.as_str());
    match &definition.settings {
        EngineSettings::Postgresql {
            host,
            port,
            database,
            sslmode,
        } => format!(
            "[[connection]]\nname = {name}\nhost = {}\nport = {port}\ndatabase = {}\nuser = {user}\nsslmode = {}\ntype = {kind}\n",
            quoted(host),
            quoted(database),
            quoted(sslmode.as_str()),
        ),
        EngineSettings::Mongodb(settings) => {
            let mut text = format!("[[connection]]\nname = {name}\nengine = \"mongodb\"\n");
            match &settings.transport {
                MongoTransport::Srv { host } => {
                    text.push_str(&format!("host = {}\nsrv = true\n", quoted(host)));
                }
                MongoTransport::Seed { host, port, tls } => text.push_str(&format!(
                    "host = {}\nport = {port}\nsrv = false\ntls = {tls}\n",
                    quoted(host)
                )),
            }
            if let Some(replica_set) = &settings.replica_set {
                text.push_str(&format!("replica_set = {}\n", quoted(replica_set)));
            }
            if let Some(database) = &settings.database {
                text.push_str(&format!("database = {}\n", quoted(database)));
            }
            if let Some(read_preference) = &settings.read_preference {
                text.push_str(&format!(
                    "read_preference = {}\n",
                    quoted(read_preference.mode.as_str())
                ));
                if !read_preference.tags.is_empty() {
                    let tags: Vec<String> = read_preference
                        .tags
                        .iter()
                        .map(|(key, value)| quoted(&format!("{key}:{value}")))
                        .collect();
                    text.push_str(&format!("read_preference_tags = [{}]\n", tags.join(", ")));
                }
            }
            text.push_str(&format!("user = {user}\ntype = {kind}\n"));
            text
        }
    }
}

fn render(definitions: &[ConnectionDefinition]) -> String {
    let mut text = String::new();
    for definition in definitions {
        text.push_str(&identity_lines(definition));
        match &definition.auth {
            AuthMode::Pgpass => text.push_str("auth = \"pgpass\"\n"),
            AuthMode::Azure => text.push_str("auth = \"azure\"\n"),
            AuthMode::Command(argv) => {
                let items: Vec<String> = argv.iter().map(|item| quoted(item)).collect();
                text.push_str(&format!(
                    "auth = \"command\"\npassword_command = [{}]\n",
                    items.join(", ")
                ));
            }
        }
        if let Some(alias) = &definition.ssh_alias {
            text.push_str(&format!("ssh_alias = {}\n", quoted(alias)));
        }
        if let Some(cost) = definition.heavy_query.cost {
            text.push_str(&format!("heavy_cost_limit = {cost}\n"));
        }
        if let Some(scan_rows) = definition.heavy_query.scan_rows {
            text.push_str(&format!("heavy_scan_limit = {scan_rows}\n"));
        }
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod integration {
    use std::path::Path;

    use super::*;
    use crate::contexts::connections::domain::types::{
        ConnectionType, HeavyQueryLimits, MongoReadPreference, MongoSettings, MongoTransport,
        ReadPreferenceMode, SslMode,
    };
    use crate::contexts::connections::infrastructure::data_sources::connection_file::parse_file;

    fn definition(name: &str, auth: AuthMode, alias: Option<&str>) -> ConnectionDefinition {
        ConnectionDefinition {
            name: name.to_string(),
            user: "alice".to_string(),
            settings: EngineSettings::Postgresql {
                host: "db.internal".to_string(),
                port: 6432,
                database: "ap\"p".to_string(),
                sslmode: SslMode::VerifyFull,
            },
            connection_type: ConnectionType::Test,
            auth,
            ssh_alias: alias.map(str::to_string),
            heavy_query: HeavyQueryLimits::default(),
        }
    }

    fn mongo_definition(name: &str, settings: MongoSettings) -> ConnectionDefinition {
        ConnectionDefinition {
            name: name.to_string(),
            user: "alice".to_string(),
            settings: EngineSettings::Mongodb(settings),
            connection_type: ConnectionType::Dev,
            auth: AuthMode::Command(vec!["/bin/echo".into(), "x".into()]),
            ssh_alias: Some("bastion".to_string()),
            heavy_query: HeavyQueryLimits::default(),
        }
    }

    fn writer_in(dir: &Path) -> TomlConnectionFileWriter {
        TomlConnectionFileWriter::new(Some(dir.join("nested").join("connections.toml")))
    }

    #[test]
    fn le_fichier_ecrit_est_relu_par_le_schema_avec_les_memes_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let definitions = vec![
            definition("a \"quoted\" name", AuthMode::Pgpass, Some("bastion")),
            definition("b", AuthMode::Azure, None),
            definition(
                "c",
                AuthMode::Command(vec!["op".into(), "read".into(), "op://v/x y".into()]),
                None,
            ),
        ];
        let path = writer_in(dir.path()).write(&definitions, false).unwrap();
        let parsed = parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let read: Vec<ConnectionDefinition> = parsed
            .entries
            .into_iter()
            .map(|entry| entry.definition.unwrap())
            .collect();
        assert_eq!(read, definitions);
    }

    #[test]
    fn les_cles_mongodb_font_l_aller_retour_par_le_fichier_ecrit() {
        let dir = tempfile::tempdir().unwrap();
        let definitions = vec![
            mongo_definition(
                "full",
                MongoSettings {
                    database: Some("shop".to_string()),
                    replica_set: Some("rs0".to_string()),
                    transport: MongoTransport::Seed {
                        host: "pl-0.atlas.scarif.test".to_string(),
                        port: 1024,
                        tls: false,
                    },
                    read_preference: Some(MongoReadPreference {
                        mode: ReadPreferenceMode::Secondary,
                        tags: vec![
                            ("nodeType".to_string(), "ANALYTICS".to_string()),
                            ("region".to_string(), "eu:west".to_string()),
                        ],
                    }),
                },
            ),
            mongo_definition(
                "atlas",
                MongoSettings {
                    database: None,
                    replica_set: None,
                    transport: MongoTransport::Srv {
                        host: "pl-0.atlas.scarif.test".to_string(),
                    },
                    read_preference: None,
                },
            ),
            mongo_definition(
                "nearest",
                MongoSettings {
                    database: None,
                    replica_set: None,
                    transport: MongoTransport::Srv {
                        host: "pl-0.atlas.scarif.test".to_string(),
                    },
                    read_preference: Some(MongoReadPreference::of(ReadPreferenceMode::Nearest)),
                },
            ),
        ];
        let path = writer_in(dir.path()).write(&definitions, false).unwrap();
        let parsed = parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let read: Vec<ConnectionDefinition> = parsed
            .entries
            .into_iter()
            .map(|entry| entry.definition.unwrap())
            .collect();
        assert_eq!(read, definitions);
    }

    #[test]
    fn les_seuils_de_requete_lourde_font_l_aller_retour_par_le_fichier_ecrit() {
        let dir = tempfile::tempdir().unwrap();
        let mut postgres = definition("shop-prod", AuthMode::Pgpass, None);
        postgres.connection_type = ConnectionType::Prod;
        postgres.heavy_query = HeavyQueryLimits {
            cost: Some(50_000),
            scan_rows: Some(200_000),
        };
        let mut mongo = mongo_definition(
            "orders-prod",
            MongoSettings {
                database: None,
                replica_set: None,
                transport: MongoTransport::Srv {
                    host: "pl-0.atlas.scarif.test".to_string(),
                },
                read_preference: None,
            },
        );
        mongo.connection_type = ConnectionType::Prod;
        mongo.heavy_query.scan_rows = Some(10);
        let definitions = vec![postgres, mongo];
        let path = writer_in(dir.path()).write(&definitions, false).unwrap();
        let parsed = parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let read: Vec<ConnectionDefinition> = parsed
            .entries
            .into_iter()
            .map(|entry| entry.definition.unwrap())
            .collect();
        assert_eq!(read, definitions);
    }

    #[test]
    fn un_fichier_existant_est_refuse_sans_force_et_reste_inchange() {
        let dir = tempfile::tempdir().unwrap();
        let writer = writer_in(dir.path());
        let path = writer
            .write(&[definition("a", AuthMode::Pgpass, None)], false)
            .unwrap();
        let before = std::fs::read_to_string(&path).unwrap();
        let error = writer
            .write(&[definition("b", AuthMode::Pgpass, None)], false)
            .unwrap_err();
        assert_eq!(error, ConnectionFileWriteError::AlreadyExists(path.clone()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    }

    #[test]
    fn force_remplace_le_fichier_existant_et_ne_laisse_aucun_fichier_temporaire() {
        let dir = tempfile::tempdir().unwrap();
        let writer = writer_in(dir.path());
        let path = writer
            .write(&[definition("a", AuthMode::Pgpass, None)], false)
            .unwrap();
        writer
            .write(&[definition("b", AuthMode::Pgpass, None)], true)
            .unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("name = \"b\"")
        );
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(leftovers.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn un_lien_symbolique_au_nom_temporaire_predictible_n_est_pas_suivi_et_le_fichier_est_en_0600()
    {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let writer = writer_in(dir.path());
        let nested = dir.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, "keep").unwrap();
        let predictable = nested.join(format!(".connections.toml.{}.tmp", std::process::id()));
        std::os::unix::fs::symlink(&victim, &predictable).unwrap();
        let path = writer
            .write(&[definition("a", AuthMode::Pgpass, None)], false)
            .unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("name = \"a\"")
        );
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
