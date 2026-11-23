use std::collections::HashMap;
use std::sync::Arc;

use crate::contexts::connections::domain::errors::ImportError;
use crate::contexts::connections::domain::repositories::{
    ConnectionFileWriter, DbeaverDefinitions, SshAliasDirectory,
};
use crate::contexts::connections::domain::services::is_azure_postgres_host;
use crate::contexts::connections::domain::types::{
    AuthMode, ConnectionDefinition, ConnectionType, DbeaverDefinition, HostBlock, ImportReport,
    ImportedConnection, SkippedConnection,
};

type TunnelKey = (String, u16, String);

pub struct ImportDbeaverConnections {
    definitions: Arc<dyn DbeaverDefinitions>,
    writer: Arc<dyn ConnectionFileWriter>,
    aliases: Arc<dyn SshAliasDirectory>,
}

struct AliasPlan {
    taken: Vec<String>,
    resolved: HashMap<TunnelKey, (String, bool)>,
    blocks: Vec<HostBlock>,
}

impl ImportDbeaverConnections {
    pub fn new(
        definitions: Arc<dyn DbeaverDefinitions>,
        writer: Arc<dyn ConnectionFileWriter>,
        aliases: Arc<dyn SshAliasDirectory>,
    ) -> Self {
        Self {
            definitions,
            writer,
            aliases,
        }
    }

    pub fn execute(&self, force: bool) -> Result<ImportReport, ImportError> {
        let read = self.definitions.read().map_err(ImportError::Source)?;
        let mut skipped = read.skipped;
        let mut definitions = Vec::new();
        for definition in read.definitions {
            match unsafe_tunnel_reason(&definition) {
                Some(reason) => skipped.push(SkippedConnection {
                    name: definition.name,
                    reason,
                }),
                None => definitions.push(definition),
            }
        }
        if definitions.is_empty() {
            return Err(ImportError::NothingToImport);
        }
        let mut plan = AliasPlan {
            taken: self.aliases.known_aliases(),
            resolved: HashMap::new(),
            blocks: Vec::new(),
        };
        let mut files = Vec::new();
        let mut imported = Vec::new();
        for definition in &definitions {
            let (file, summary) = self.convert(definition, &mut plan);
            files.push(file);
            imported.push(summary);
        }
        let path = self.writer.write(&files, force)?;
        Ok(ImportReport {
            path,
            imported,
            skipped,
            host_blocks: plan.blocks,
        })
    }

    fn convert(
        &self,
        definition: &DbeaverDefinition,
        plan: &mut AliasPlan,
    ) -> (ConnectionDefinition, ImportedConnection) {
        let auth = if definition.pgpass_auth && is_azure_postgres_host(&definition.host) {
            AuthMode::Azure
        } else {
            AuthMode::Pgpass
        };
        let (connection_type, type_defaulted_from) =
            match ConnectionType::from_name(&definition.connection_type) {
                Some(known) => (known, None),
                None => (
                    ConnectionType::Prod,
                    Some(definition.connection_type.clone()),
                ),
            };
        let alias = definition
            .tunnel
            .as_ref()
            .map(|tunnel| self.alias_of(tunnel_key(tunnel), definition, plan));
        let file = ConnectionDefinition {
            name: definition.name.clone(),
            host: definition.host.clone(),
            port: definition.port,
            database: definition.database.clone(),
            user: definition.user.clone(),
            sslmode: definition.sslmode,
            connection_type,
            auth: auth.clone(),
            ssh_alias: alias.as_ref().map(|(alias, _)| alias.clone()),
        };
        let summary = ImportedConnection {
            name: definition.name.clone(),
            auth,
            ssh_alias: file.ssh_alias.clone(),
            alias_proposed: alias.is_some_and(|(_, proposed)| proposed),
            type_defaulted_from,
        };
        (file, summary)
    }

    fn alias_of(
        &self,
        key: TunnelKey,
        definition: &DbeaverDefinition,
        plan: &mut AliasPlan,
    ) -> (String, bool) {
        if let Some(known) = plan.resolved.get(&key) {
            return known.clone();
        }
        let (host, port, user) = key.clone();
        let outcome = match self.aliases.alias_for(&host, port, &user) {
            Some(existing) => (existing, false),
            None => {
                let alias = unique_alias(&host, &plan.taken);
                plan.taken.push(alias.clone());
                plan.blocks.push(HostBlock {
                    alias: alias.clone(),
                    host,
                    port,
                    user,
                    identity_file: definition
                        .tunnel
                        .as_ref()
                        .and_then(|tunnel| tunnel.key_path.clone()),
                });
                (alias, true)
            }
        };
        plan.resolved.insert(key, outcome.clone());
        outcome
    }
}

fn unsafe_tunnel_reason(definition: &DbeaverDefinition) -> Option<String> {
    let tunnel = definition.tunnel.as_ref()?;
    let key = tunnel
        .key_path
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let unsafe_value = [Some(&tunnel.host), Some(&tunnel.user), key.as_ref()]
        .into_iter()
        .flatten()
        .any(|value| {
            value.starts_with('-')
                || value
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control())
        });
    unsafe_value.then(|| {
        "its SSH tunnel host, user or key path contains whitespace, control characters or starts with a dash, so it cannot be written to an OpenSSH config safely; fix it in DBeaver and import again".to_string()
    })
}

fn tunnel_key(tunnel: &crate::contexts::connections::domain::types::DbeaverTunnel) -> TunnelKey {
    (tunnel.host.clone(), tunnel.port, tunnel.user.clone())
}

fn unique_alias(host: &str, taken: &[String]) -> String {
    let base: String = host
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let base = if base.is_empty() || base.starts_with('-') {
        format!("bastion{base}")
    } else {
        base
    };
    let mut candidate = base.clone();
    let mut counter = 2;
    while taken.contains(&candidate) {
        candidate = format!("{base}-{counter}");
        counter += 1;
    }
    candidate
}

#[cfg(test)]
mod unit {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::contexts::connections::domain::errors::ConnectionFileWriteError;
    use crate::contexts::connections::domain::types::{
        DbeaverRead, DbeaverTunnel, SkippedConnection, SslMode,
    };

    struct Source(DbeaverRead);

    impl DbeaverDefinitions for Source {
        fn read(&self) -> Result<DbeaverRead, String> {
            Ok(self.0.clone())
        }
    }

    #[derive(Default)]
    struct Recorder {
        written: Mutex<Vec<ConnectionDefinition>>,
        existing: bool,
    }

    impl ConnectionFileWriter for Recorder {
        fn path(&self) -> Result<PathBuf, ConnectionFileWriteError> {
            Ok(PathBuf::from("/config/connections.toml"))
        }

        fn write(
            &self,
            definitions: &[ConnectionDefinition],
            overwrite: bool,
        ) -> Result<PathBuf, ConnectionFileWriteError> {
            if self.existing && !overwrite {
                return Err(ConnectionFileWriteError::AlreadyExists(PathBuf::from(
                    "/config/connections.toml",
                )));
            }
            *self.written.lock().unwrap() = definitions.to_vec();
            self.path()
        }
    }

    struct Aliases {
        known: Vec<(String, u16, String, String)>,
        taken: Vec<String>,
    }

    impl SshAliasDirectory for Aliases {
        fn alias_for(&self, host: &str, port: u16, user: &str) -> Option<String> {
            self.known
                .iter()
                .find(|known| known.0 == host && known.1 == port && known.2 == user)
                .map(|known| known.3.clone())
        }

        fn known_aliases(&self) -> Vec<String> {
            self.taken.clone()
        }

        fn defines(&self, _alias: &str) -> bool {
            true
        }
    }

    fn definition(name: &str, host: &str) -> DbeaverDefinition {
        DbeaverDefinition {
            name: name.to_string(),
            host: host.to_string(),
            port: 5432,
            database: "app".to_string(),
            user: "alice".to_string(),
            sslmode: SslMode::Prefer,
            connection_type: "dev".to_string(),
            pgpass_auth: false,
            tunnel: None,
        }
    }

    fn tunneled(name: &str, bastion: &str) -> DbeaverDefinition {
        DbeaverDefinition {
            tunnel: Some(DbeaverTunnel {
                host: bastion.to_string(),
                port: 22,
                user: "jump".to_string(),
                key_path: Some(PathBuf::from("/keys/id_ed25519")),
            }),
            ..definition(name, "db.internal")
        }
    }

    fn run(
        definitions: Vec<DbeaverDefinition>,
        aliases: Aliases,
        recorder: &Arc<Recorder>,
        force: bool,
    ) -> Result<ImportReport, ImportError> {
        ImportDbeaverConnections::new(
            Arc::new(Source(DbeaverRead {
                definitions,
                skipped: vec![SkippedConnection {
                    name: "legacy".to_string(),
                    reason: "engine mysql is not supported".to_string(),
                }],
            })),
            recorder.clone(),
            Arc::new(aliases),
        )
        .execute(force)
    }

    fn no_aliases() -> Aliases {
        Aliases {
            known: Vec::new(),
            taken: Vec::new(),
        }
    }

    #[test]
    fn l_authentification_native_devient_pgpass_et_pgpass_sur_un_hote_azure_devient_azure() {
        let recorder = Arc::new(Recorder::default());
        let mut azure = definition("shop", "shop.postgres.database.azure.com");
        azure.pgpass_auth = true;
        let mut plain_pgpass = definition("plain", "db.example");
        plain_pgpass.pgpass_auth = true;
        let report = run(
            vec![definition("native", "db.example"), plain_pgpass, azure],
            no_aliases(),
            &recorder,
            false,
        )
        .unwrap();
        let written = recorder.written.lock().unwrap().clone();
        assert_eq!(
            written.iter().map(|d| d.auth.clone()).collect::<Vec<_>>(),
            vec![AuthMode::Pgpass, AuthMode::Pgpass, AuthMode::Azure]
        );
        assert_eq!(report.imported.len(), 3);
        assert_eq!(report.skipped.len(), 1);
    }

    #[test]
    fn un_type_inconnu_devient_prod_et_l_original_est_note() {
        let recorder = Arc::new(Recorder::default());
        let mut staging = definition("staging", "db.example");
        staging.connection_type = "staging".to_string();
        let report = run(vec![staging], no_aliases(), &recorder, false).unwrap();
        assert_eq!(
            recorder.written.lock().unwrap()[0].connection_type,
            ConnectionType::Prod
        );
        assert_eq!(
            report.imported[0].type_defaulted_from.as_deref(),
            Some("staging")
        );
    }

    #[test]
    fn un_alias_existant_qui_correspond_est_reutilise_sans_bloc_propose() {
        let recorder = Arc::new(Recorder::default());
        let aliases = Aliases {
            known: vec![(
                "bastion.corp".into(),
                22,
                "jump".into(),
                "corp-bastion".into(),
            )],
            taken: vec!["corp-bastion".into()],
        };
        let report = run(
            vec![tunneled("a", "bastion.corp")],
            aliases,
            &recorder,
            false,
        )
        .unwrap();
        assert_eq!(
            report.imported[0].ssh_alias.as_deref(),
            Some("corp-bastion")
        );
        assert!(!report.imported[0].alias_proposed);
        assert!(report.host_blocks.is_empty());
    }

    #[test]
    fn un_bastion_inconnu_recoit_un_alias_unique_et_un_seul_bloc_pour_plusieurs_connexions() {
        let recorder = Arc::new(Recorder::default());
        let aliases = Aliases {
            known: Vec::new(),
            taken: vec!["bastion.corp".into()],
        };
        let report = run(
            vec![tunneled("a", "bastion.corp"), tunneled("b", "bastion.corp")],
            aliases,
            &recorder,
            false,
        )
        .unwrap();
        assert_eq!(report.host_blocks.len(), 1);
        let block = &report.host_blocks[0];
        assert_eq!(block.alias, "bastion.corp-2");
        assert_eq!(block.identity_file, Some(PathBuf::from("/keys/id_ed25519")));
        let written = recorder.written.lock().unwrap().clone();
        assert!(
            written
                .iter()
                .all(|d| d.ssh_alias.as_deref() == Some("bastion.corp-2"))
        );
    }

    #[test]
    fn un_fichier_existant_sans_force_fait_echouer_l_import_et_avec_force_il_est_remplace() {
        let existing = Arc::new(Recorder {
            existing: true,
            ..Recorder::default()
        });
        let error = run(vec![definition("a", "db")], no_aliases(), &existing, false).unwrap_err();
        assert!(matches!(
            error,
            ImportError::File(ConnectionFileWriteError::AlreadyExists(_))
        ));
        assert!(existing.written.lock().unwrap().is_empty());
        run(vec![definition("a", "db")], no_aliases(), &existing, true).unwrap();
        assert_eq!(existing.written.lock().unwrap().len(), 1);
    }

    #[test]
    fn sans_connexion_postgresql_l_import_s_arrete_sans_rien_ecrire() {
        let recorder = Arc::new(Recorder::default());
        let error = run(Vec::new(), no_aliases(), &recorder, false).unwrap_err();
        assert_eq!(error, ImportError::NothingToImport);
        assert!(recorder.written.lock().unwrap().is_empty());
    }

    #[test]
    fn un_nom_d_alias_est_derive_de_l_hote_du_bastion_sans_caractere_dangereux() {
        assert_eq!(unique_alias("bastion.corp", &[]), "bastion.corp");
        assert_eq!(unique_alias("10.0.0.5", &[]), "10.0.0.5");
        assert_eq!(unique_alias("my host;x", &[]), "my-host-x");
        assert_eq!(unique_alias("-bad", &[]), "bastion-bad");
    }

    fn unsafe_tunnel(host: &str, user: &str, key: &str) -> DbeaverDefinition {
        DbeaverDefinition {
            tunnel: Some(DbeaverTunnel {
                host: host.to_string(),
                port: 22,
                user: user.to_string(),
                key_path: Some(PathBuf::from(key)),
            }),
            ..definition("risky", "db.internal")
        }
    }

    #[test]
    fn un_tunnel_dont_l_hote_l_utilisateur_ou_la_cle_est_dangereux_est_ecarte_sans_bloc_ssh() {
        for (host, user, key) in [
            (
                "bastion\n  ProxyCommand touch /tmp/pwned",
                "jump",
                "/keys/id",
            ),
            ("bastion", "jump\nProxyCommand evil", "/keys/id"),
            ("bastion", "jump", "/keys/id\nProxyCommand evil"),
            ("-oProxyCommand=evil", "jump", "/keys/id"),
            ("bastion", "-oProxyCommand=evil", "/keys/id"),
            ("bastion", "jump", "-oProxyCommand=evil"),
            ("bast ion", "jump", "/keys/id"),
            ("bastion", "ju\u{7}mp", "/keys/id"),
        ] {
            let recorder = Arc::new(Recorder::default());
            let report = run(
                vec![
                    definition("safe", "db.example"),
                    unsafe_tunnel(host, user, key),
                ],
                no_aliases(),
                &recorder,
                false,
            )
            .unwrap();
            assert!(report.host_blocks.is_empty(), "{host:?} {user:?} {key:?}");
            assert_eq!(report.imported.len(), 1);
            let skipped = report
                .skipped
                .iter()
                .find(|skipped| skipped.name == "risky")
                .expect("the risky connection is reported as skipped");
            assert!(skipped.reason.contains("SSH tunnel"), "{}", skipped.reason);
            assert!(
                !skipped.reason.contains("ProxyCommand"),
                "{}",
                skipped.reason
            );
            let written = recorder.written.lock().unwrap().clone();
            assert_eq!(written.len(), 1);
            assert_eq!(written[0].name, "safe");
        }
    }

    #[test]
    fn un_hote_de_tunnel_ordinaire_avec_un_tiret_au_milieu_reste_importe() {
        let recorder = Arc::new(Recorder::default());
        let report = run(
            vec![unsafe_tunnel("bastion-eu.corp", "jump", "/keys/id_ed25519")],
            no_aliases(),
            &recorder,
            false,
        )
        .unwrap();
        assert_eq!(report.host_blocks.len(), 1);
    }
}
