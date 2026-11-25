use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::contexts::connections::domain::errors::{AccessTokenError, ConnectionError};
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::services::resolve_connection;
use crate::contexts::connections::domain::types::{
    ConnectParams, ConnectionEntry, ConnectionStatus, Discovery, Secret, SshAuth, SslMode,
    TunnelParams,
};
use crate::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use crate::contexts::connections::infrastructure::data_sources::dbeaver::{
    discover as discover_with, discover_in as discover_in_with,
};
use crate::contexts::connections::infrastructure::repositories::DbeaverConnectionCatalog;
use crate::contexts::connections::presentation;
use crate::contexts::connections::use_cases::ListConnections;
use rmcp::model::CallToolResult;

#[cfg(unix)]
mod azure;

struct NoTokens;

impl AzureSignInTokenProvider for NoTokens {
    fn access_token(&self) -> Result<Secret, AccessTokenError> {
        Err(AccessTokenError::CliNotInstalled)
    }

    fn renewed_access_token(&self) -> Result<Secret, AccessTokenError> {
        Err(AccessTokenError::CliNotInstalled)
    }
}

fn discover(home: Option<&Path>) -> Discovery {
    discover_in(home, None)
}

fn discover_in(home: Option<&Path>, database: Option<&str>) -> Discovery {
    discover_in_with(home, &NoTokens, None, database)
}

fn discover_pgpass(home: Option<&Path>, pgpass: &Path, database: Option<&str>) -> Discovery {
    discover_in_with(home, &NoTokens, Some(pgpass), database)
}

fn write_project(root: &Path, project: &str, suffix: &str, sources: &Value, creds: Option<&Value>) {
    let dir = root.join("workspace6").join(project).join(".dbeaver");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("data-sources{suffix}.json")),
        serde_json::to_vec_pretty(sources).unwrap(),
    )
    .unwrap();
    if let Some(creds) = creds {
        let iv: [u8; 16] = rand::random();
        fs::write(
            dir.join(format!("credentials-config{suffix}.json")),
            encrypt_for_tests(&creds.to_string(), iv),
        )
        .unwrap();
    }
}

fn pg_connection(name: &str, extra: Value) -> Value {
    let mut connection = json!({
        "provider": "postgresql",
        "driver": "postgres-jdbc",
        "name": name,
        "save-password": true,
        "configuration": {
            "host": "db.internal.example",
            "port": "5433",
            "database": "shop",
            "type": "dev",
            "auth-model": "native"
        }
    });
    merge(&mut connection, extra);
    connection
}

fn merge(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                merge(target.entry(key).or_insert(Value::Null), value);
            }
        }
        (target, patch) => *target = patch,
    }
}

fn sources(connections: Value) -> Value {
    json!({ "folders": {}, "connection-types": {}, "connections": connections })
}

fn creds(id: &str, user: &str, password: &str) -> Value {
    json!({ id: { "#connection": { "user": user, "password": password } } })
}

fn config(root: &Path) -> PathBuf {
    root.to_path_buf()
}

fn find_connection(home: &Path, name: &str) -> Result<ConnectParams, ConnectionError> {
    resolve_connection(&discover(Some(home)), name)
}

async fn list_connections(home: &Path) -> CallToolResult {
    let catalog = DbeaverConnectionCatalog::new(Some(home.to_path_buf()), Arc::new(NoTokens), None);
    presentation::list_connections(&ListConnections::new(Arc::new(catalog))).await
}

fn only(root: &Path) -> ConnectionEntry {
    let mut discovery = discover(Some(root));
    assert_eq!(discovery.connections.len(), 1, "{discovery:?}");
    discovery.connections.remove(0)
}

fn reason(status: &ConnectionStatus) -> &str {
    match status {
        ConnectionStatus::Unavailable { reason } | ConnectionStatus::Unsupported { reason } => {
            reason
        }
        ConnectionStatus::Available => panic!("expected a reason"),
    }
}

mod integration {
    use super::*;

    #[test]
    fn des_identifiants_chiffres_donnent_une_connexion_disponible() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&creds("pg-1", "reader", "s3cret-pw")),
        );
        let entry = only(home.path());
        assert_eq!(entry.name, "Shop");
        assert_eq!(entry.project, "General");
        assert_eq!(entry.engine, "postgresql");
        assert_eq!(entry.connection_type, "dev");
        assert_eq!(entry.database.as_deref(), Some("shop"));
        assert_eq!(entry.status, ConnectionStatus::Available);
        let params = entry.params.unwrap();
        assert_eq!(params.host, "db.internal.example");
        assert_eq!(params.port, 5433);
        assert_eq!(params.database, "shop");
        assert_eq!(params.user, "reader");
        assert_eq!(params.password.expose(), "s3cret-pw");
        assert_eq!(params.sslmode, SslMode::Prefer);
    }

    #[test]
    fn un_fichier_data_sources_secondaire_est_liste_avec_celui_par_defaut() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&creds("pg-1", "a", "b")),
        );
        write_project(
            home.path(),
            "Second",
            "-2",
            &sources(json!({ "pg-2": pg_connection("Billing", json!({})) })),
            Some(&creds("pg-2", "c", "d")),
        );
        let discovery = discover(Some(home.path()));
        let mut names: Vec<_> = discovery
            .connections
            .iter()
            .map(|entry| (entry.name.as_str(), entry.status.clone()))
            .collect();
        names.sort_by(|a, b| a.0.cmp(b.0));
        assert_eq!(
            names,
            vec![
                ("Billing", ConnectionStatus::Available),
                ("Shop", ConnectionStatus::Available)
            ]
        );
    }

    #[test]
    fn l_utilisateur_et_le_mot_de_passe_en_clair_priment_sur_le_fichier_d_identifiants() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "user": "plain-user", "password": "plain-pw" }
        })) })),
            Some(&creds("pg-1", "file-user", "file-pw")),
        );
        let params = only(home.path()).params.unwrap();
        assert_eq!(params.user, "plain-user");
        assert_eq!(params.password.expose(), "plain-pw");
    }

    #[test]
    fn un_mot_de_passe_enregistre_sans_entree_d_identifiants_rend_la_connexion_indisponible() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&json!({})),
        );
        let entry = only(home.path());
        assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
        let text = reason(&entry.status);
        assert!(text.contains("master password"), "{text}");
        assert!(text.contains("secure storage"), "{text}");
        assert!(entry.params.is_none());
    }

    #[test]
    fn un_mot_de_passe_non_enregistre_rend_la_connexion_indisponible() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({ "save-password": false })) })),
            None,
        );
        let entry = only(home.path());
        assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
        assert!(reason(&entry.status).contains("no saved password"));
    }

    #[test]
    fn un_data_sources_json_invalide_n_empeche_pas_de_charger_les_autres_projets() {
        let home = TempDir::new().unwrap();
        let broken = home.path().join("workspace6/Broken/.dbeaver");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("data-sources.json"), [0x8f, 0x01, 0xfe, 0x42]).unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&creds("pg-1", "a", "b")),
        );
        let discovery = discover(Some(home.path()));
        assert_eq!(discovery.connections.len(), 2);
        let broken_entry = discovery
            .connections
            .iter()
            .find(|entry| entry.project == "Broken")
            .unwrap();
        assert!(matches!(
            broken_entry.status,
            ConnectionStatus::Unavailable { .. }
        ));
        assert!(!reason(&broken_entry.status).is_empty());
        assert!(
            discovery
                .connections
                .iter()
                .any(|entry| entry.name == "Shop" && entry.status == ConnectionStatus::Available)
        );
    }

    #[test]
    fn un_tunnel_ssh_desactive_est_ignore() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "handlers": { "ssh_tunnel": { "type": "TUNNEL", "enabled": false } } }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        assert_eq!(only(home.path()).status, ConnectionStatus::Available);
    }

    #[test]
    fn un_autre_moteur_est_non_supporte_et_nomme_le_moteur() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "my-1": pg_connection("Legacy", json!({ "provider": "mysql" })) })),
            None,
        );
        let entry = only(home.path());
        assert!(matches!(entry.status, ConnectionStatus::Unsupported { .. }));
        assert!(reason(&entry.status).contains("mysql"));
        assert_eq!(entry.engine, "mysql");
    }

    #[test]
    fn un_modele_d_authentification_non_natif_est_non_supporte() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "auth-model": "postgres_kerberos" }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        let entry = only(home.path());
        assert!(matches!(entry.status, ConnectionStatus::Unsupported { .. }));
        assert!(reason(&entry.status).contains("postgres_kerberos"));
    }

    #[test]
    fn une_variable_est_resolue_depuis_l_environnement_quand_elle_est_definie() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "host": "${PGMCP_TEST_HOST_SET:localhost}" }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        unsafe { std::env::set_var("PGMCP_TEST_HOST_SET", "env-host.example") };
        let params = only(home.path()).params.unwrap();
        assert_eq!(params.host, "env-host.example");
    }

    #[test]
    fn une_variable_absente_prend_sa_valeur_par_defaut() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "host": "${PGMCP_TEST_HOST_UNSET:localhost}" }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        assert_eq!(only(home.path()).params.unwrap().host, "localhost");
    }

    #[test]
    fn une_variable_non_resolue_sans_defaut_rend_la_connexion_indisponible() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "host": "${PGMCP_TEST_HOST_NEVER}" }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        let entry = only(home.path());
        assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
        assert!(reason(&entry.status).contains("PGMCP_TEST_HOST_NEVER"));
    }

    #[test]
    fn le_mode_url_extrait_hote_port_base_et_sslmode() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": {
                "configurationType": "URL",
                "host": null,
                "port": null,
                "database": null,
                "url": "jdbc:postgresql://url-host.example:6543/urldb?sslmode=require&ApplicationName=x"
            }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        let params = only(home.path()).params.unwrap();
        assert_eq!(params.host, "url-host.example");
        assert_eq!(params.port, 6543);
        assert_eq!(params.database, "urldb");
        assert_eq!(params.sslmode, SslMode::Require);
    }

    #[test]
    fn le_sslmode_est_lu_dans_les_proprietes_du_fournisseur() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "provider-properties": { "sslmode": "verify-full" } }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        assert_eq!(
            only(home.path()).params.unwrap().sslmode,
            SslMode::VerifyFull
        );
    }

    #[test]
    fn l_affichage_debug_des_parametres_ne_montre_jamais_le_mot_de_passe() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&creds("pg-1", "reader", "hunter2-canary")),
        );
        let entry = only(home.path());
        let params = entry.params.clone().unwrap();
        assert!(!format!("{params:?}").contains("hunter2-canary"));
        assert!(!format!("{entry:?}").contains("hunter2-canary"));
    }

    #[test]
    fn un_dossier_dbeaver_absent_donne_une_liste_vide_avec_un_indice() {
        let home = TempDir::new().unwrap();
        let missing = home.path().join("nowhere");
        let discovery = discover(Some(&missing));
        assert!(discovery.connections.is_empty());
        assert!(discovery.hint.is_some());
    }

    #[test]
    fn les_noms_en_double_recoivent_un_suffixe_stable() {
        let home = TempDir::new().unwrap();
        for project in ["A", "B"] {
            write_project(
                home.path(),
                project,
                "",
                &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
                Some(&creds("pg-1", "a", "b")),
            );
        }
        let first: Vec<String> = discover(Some(home.path()))
            .connections
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        let second: Vec<String> = discover(Some(home.path()))
            .connections
            .into_iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_ne!(first[0], first[1]);
        assert!(first.iter().all(|name| name.starts_with("Shop (")));
    }

    #[test]
    fn la_recherche_signale_une_connexion_inconnue_ou_indisponible() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({
                "pg-1": pg_connection("Shop", json!({})),
                "pg-2": pg_connection("Locked", json!({ "save-password": false }))
            })),
            Some(&creds("pg-1", "a", "b")),
        );
        let config = config(home.path());

        match find_connection(&config, "Nope").unwrap_err() {
            ConnectionError::Unknown { name, available } => {
                assert_eq!(name, "Nope");
                assert_eq!(available, vec!["Shop".to_string()]);
            }
            other => panic!("unexpected error {other:?}"),
        }

        match find_connection(&config, "Locked").unwrap_err() {
            ConnectionError::Unavailable { reason } => {
                assert!(reason.contains("no saved password"), "{reason}");
            }
            other => panic!("unexpected error {other:?}"),
        }

        assert_eq!(find_connection(&config, "Shop").unwrap().user, "a");
    }

    #[test]
    fn la_recherche_relit_l_espace_de_travail_a_chaque_appel() {
        let home = TempDir::new().unwrap();
        let config = config(home.path());
        assert!(find_connection(&config, "Shop").is_err());
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({})) })),
            Some(&creds("pg-1", "a", "b")),
        );
        assert!(find_connection(&config, "Shop").is_ok());
    }

    #[tokio::test]
    async fn la_liste_des_connexions_n_expose_ni_hote_ni_utilisateur_ni_mot_de_passe() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({
                "pg-1": pg_connection("Shop", json!({ "folder": "Prod" })),
                "my-1": pg_connection("Legacy", json!({ "provider": "mysql" }))
            })),
            Some(&creds("pg-1", "canary-user", "canary-password")),
        );
        let result = list_connections(&config(home.path())).await;
        let text = serde_json::to_string(&result).unwrap();
        for forbidden in [
            "db.internal.example",
            "canary-user",
            "canary-password",
            "5433",
        ] {
            assert!(!text.contains(forbidden), "{forbidden} leaked in {text}");
        }
        let body: Value = serde_json::from_str(
            result.content[0]
                .as_text()
                .map(|text| text.text.as_str())
                .unwrap(),
        )
        .unwrap();
        let connections = body["connections"].as_array().unwrap();
        assert_eq!(connections.len(), 2);
        let shop = connections.iter().find(|c| c["name"] == "Shop").unwrap();
        assert_eq!(shop["status"], "available");
        assert_eq!(shop["engine"], "postgresql");
        assert_eq!(shop["type"], "dev");
        assert_eq!(shop["database"], "shop");
        assert_eq!(shop["folder"], "Prod");
        assert!(shop.get("reason").is_none());
        let legacy = connections.iter().find(|c| c["name"] == "Legacy").unwrap();
        assert_eq!(legacy["status"], "unsupported");
        assert!(legacy["reason"].as_str().unwrap().contains("mysql"));
    }

    fn key_file(home: &TempDir) -> PathBuf {
        let path = home.path().join("keys").join("id_bastion");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "synthetic key material").unwrap();
        path
    }

    fn tunnel(properties: Value) -> Value {
        let mut base = json!({
            "authType": "PUBLIC_KEY",
            "host": "bastion.example",
            "port": 2228.0,
            "implementation": "sshj",
            "jumpServer.count": 0.0,
            "localHost": "",
            "shareTunnels": true
        });
        merge(&mut base, properties);
        json!({
            "configuration": {
                "handlers": {
                    "ssh_tunnel": {
                        "type": "TUNNEL",
                        "enabled": true,
                        "save-password": true,
                        "properties": base
                    }
                }
            }
        })
    }

    fn tunnel_creds(id: &str, ssh: Value) -> Value {
        json!({ id: {
        "#connection": { "user": "reader", "password": "db-pw" },
        "network/ssh_tunnel": ssh
    } })
    }

    fn tunneled(home: &TempDir, properties: Value, ssh: Value) -> ConnectionEntry {
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", tunnel(properties)) })),
            Some(&tunnel_creds("pg-1", ssh)),
        );
        only(home.path())
    }

    fn tunnel_of(entry: ConnectionEntry) -> TunnelParams {
        assert_eq!(entry.status, ConnectionStatus::Available, "{entry:?}");
        entry.params.unwrap().tunnel.expect("tunnel params")
    }

    #[test]
    fn une_connexion_tunnelisee_par_cle_publique_est_disponible() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        let entry = tunneled(
            &home,
            json!({ "keyPath": key.to_str().unwrap() }),
            json!({ "user": "ssh-user", "password": "key-passphrase" }),
        );
        let params = entry.params.clone().unwrap();
        assert_eq!(params.host, "db.internal.example");
        assert_eq!(params.port, 5433);
        assert_eq!(params.password.expose(), "db-pw");
        let tunnel = tunnel_of(entry);
        assert_eq!(tunnel.hops[0].host, "bastion.example");
        assert_eq!(tunnel.hops[0].port, 2228);
        assert_eq!(tunnel.hops[0].user, "ssh-user");
        assert_eq!(
            tunnel.hops[0].auth,
            SshAuth::PublicKey {
                key_path: key,
                passphrase: Some(Secret::new("key-passphrase")),
            }
        );
        assert_eq!(tunnel.remote_host, "db.internal.example");
        assert_eq!(tunnel.remote_port, 5433);
        assert!(tunnel.hops[0].host_key.is_verified());
    }

    #[test]
    fn une_cle_publique_sans_phrase_secrete_enregistree_est_disponible() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        let tunnel = tunnel_of(tunneled(
            &home,
            json!({ "keyPath": key.to_str().unwrap(), "jumpServer.count": 0 }),
            json!({ "user": "ssh-user" }),
        ));
        assert_eq!(
            tunnel.hops[0].auth,
            SshAuth::PublicKey {
                key_path: key,
                passphrase: None
            }
        );
    }

    #[test]
    fn un_chemin_de_cle_factice_rend_la_connexion_indisponible() {
        let home = TempDir::new().unwrap();
        let entry = tunneled(
            &home,
            json!({ "keyPath": "/Users/CHANGEME/.ssh/id" }),
            json!({ "user": "ssh-user", "password": "key-passphrase" }),
        );
        assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
        assert_eq!(
            reason(&entry.status),
            "SSH key file not found: /Users/CHANGEME/.ssh/id"
        );
        assert!(entry.params.is_none());
    }

    #[test]
    fn un_serveur_de_rebond_est_non_supporte() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        let entry = tunneled(
            &home,
            json!({ "keyPath": key.to_str().unwrap(), "jumpServer.count": 1 }),
            json!({ "user": "ssh-user" }),
        );
        assert_eq!(
            entry.status,
            ConnectionStatus::Unsupported {
                reason: "SSH jump servers are not supported yet".to_string()
            }
        );
    }

    #[test]
    fn l_authentification_ssh_par_mot_de_passe_enregistre_est_disponible() {
        let home = TempDir::new().unwrap();
        let tunnel = tunnel_of(tunneled(
            &home,
            json!({ "authType": "PASSWORD", "port": "2222" }),
            json!({ "user": "ssh-user", "password": "ssh-pw" }),
        ));
        assert_eq!(tunnel.hops[0].port, 2222);
        assert_eq!(
            tunnel.hops[0].auth,
            SshAuth::Password(Secret::new("ssh-pw"))
        );
    }

    #[test]
    fn l_authentification_ssh_par_mot_de_passe_non_enregistre_est_indisponible() {
        let home = TempDir::new().unwrap();
        let entry = tunneled(
            &home,
            json!({ "authType": "PASSWORD" }),
            json!({ "user": "ssh-user" }),
        );
        assert!(matches!(entry.status, ConnectionStatus::Unavailable { .. }));
        assert!(reason(&entry.status).contains("SSH password"), "{entry:?}");
    }

    #[test]
    fn un_utilisateur_ssh_absent_rend_la_connexion_indisponible() {
        let home = TempDir::new().unwrap();
        let entry = tunneled(
            &home,
            json!({ "authType": "AGENT" }),
            json!({ "password": "ssh-pw" }),
        );
        assert_eq!(
            entry.status,
            ConnectionStatus::Unavailable {
                reason: "SSH user not saved in DBeaver".to_string()
            }
        );
    }

    #[test]
    fn l_authentification_par_agent_ssh_est_disponible() {
        let home = TempDir::new().unwrap();
        let tunnel = tunnel_of(tunneled(
            &home,
            json!({ "authType": "AGENT", "port": null }),
            json!({ "user": "ssh-user" }),
        ));
        assert_eq!(tunnel.hops[0].auth, SshAuth::Agent);
        assert_eq!(tunnel.hops[0].port, 22);
    }

    #[test]
    fn contourner_la_verification_d_hote_desactive_le_controle_de_cle() {
        for bypass in [json!(true), json!("true")] {
            let home = TempDir::new().unwrap();
            let tunnel = tunnel_of(tunneled(
                &home,
                json!({ "authType": "AGENT", "bypassHostVerification": bypass }),
                json!({ "user": "ssh-user" }),
            ));
            assert!(!tunnel.hops[0].host_key.is_verified());
        }
        let home = TempDir::new().unwrap();
        let tunnel = tunnel_of(tunneled(
            &home,
            json!({ "authType": "AGENT", "bypassHostVerification": false }),
            json!({ "user": "ssh-user" }),
        ));
        assert!(tunnel.hops[0].host_key.is_verified());
    }

    #[test]
    fn l_utilisateur_et_le_mot_de_passe_du_tunnel_priment_sur_le_fichier_d_identifiants() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", merge_into(
            tunnel(json!({ "authType": "PASSWORD", "remoteHost": "10.0.0.7" })),
            json!({ "configuration": { "handlers": { "ssh_tunnel": {
                "user": "handler-user", "password": "handler-pw"
            } } } }),
        )) })),
            Some(&tunnel_creds(
                "pg-1",
                json!({ "user": "file-user", "password": "file-pw" }),
            )),
        );
        let tunnel = tunnel_of(only(home.path()));
        assert_eq!(tunnel.hops[0].user, "handler-user");
        assert_eq!(
            tunnel.hops[0].auth,
            SshAuth::Password(Secret::new("handler-pw"))
        );
        assert_eq!(tunnel.remote_host, "10.0.0.7");
        assert_eq!(tunnel.remote_port, 5433);
    }

    fn merge_into(mut target: Value, patch: Value) -> Value {
        merge(&mut target, patch);
        target
    }

    #[test]
    fn les_champs_du_tunnel_resolvent_les_variables() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        let key_dir = key.parent().unwrap().to_str().unwrap().to_string();
        let tunnel = tunnel_of(tunneled(
            &home,
            json!({
                "host": "${PGMCP_TEST_SSH_HOST_UNSET:bastion.default}",
                "port": "${PGMCP_TEST_SSH_PORT_UNSET:2200}",
                "keyPath": format!("${{PGMCP_TEST_SSH_KEYDIR_UNSET:{key_dir}}}/id_bastion")
            }),
            json!({ "user": "${PGMCP_TEST_SSH_USER_UNSET:ssh-default}" }),
        ));
        assert_eq!(tunnel.hops[0].host, "bastion.default");
        assert_eq!(tunnel.hops[0].port, 2200);
        assert_eq!(tunnel.hops[0].user, "ssh-default");
        assert_eq!(
            tunnel.hops[0].auth,
            SshAuth::PublicKey {
                key_path: key,
                passphrase: None
            }
        );
    }

    #[test]
    fn un_proxy_active_est_non_supporte() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({ "pg-1": pg_connection("Shop", json!({
            "configuration": { "handlers": { "socks_proxy": { "type": "PROXY", "enabled": true } } }
        })) })),
            Some(&creds("pg-1", "a", "b")),
        );
        let entry = only(home.path());
        assert!(matches!(entry.status, ConnectionStatus::Unsupported { .. }));
        assert!(reason(&entry.status).contains("proxy"));
    }

    #[test]
    fn l_affichage_debug_d_un_tunnel_ne_montre_jamais_les_secrets() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        let entry = tunneled(
            &home,
            json!({ "keyPath": key.to_str().unwrap() }),
            json!({ "user": "ssh-user", "password": "passphrase-canary" }),
        );
        let params = entry.params.clone().unwrap();
        let tunnel = params.tunnel.clone().unwrap();
        for text in [
            format!("{params:?}"),
            format!("{tunnel:?}"),
            format!("{entry:?}"),
        ] {
            assert!(!text.contains("passphrase-canary"), "{text}");
            assert!(!text.contains("db-pw"), "{text}");
        }
        let home = TempDir::new().unwrap();
        let entry = tunneled(
            &home,
            json!({ "authType": "PASSWORD" }),
            json!({ "user": "ssh-user", "password": "ssh-pw-canary" }),
        );
        assert!(!format!("{entry:?}").contains("ssh-pw-canary"));
    }

    #[test]
    fn pgpass_fournit_le_mot_de_passe_de_la_base() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        let pgpass = home.path().join("pgpass-dbeaver-test");
        fs::write(
        &pgpass,
        "# synthetic\nbastion.example:*:*:*:wrong-host\ndb.internal.example:5433:shop:writer:wrong-user\n*:5433:shop:reader:p\\:gp\\\\ss\n",
    )
    .unwrap();
        set_mode(&pgpass, 0o600);
        let pgpass_model = |extra: Value| {
            merge_into(
                json!({ "configuration": { "auth-model": "postgres_pgpass" } }),
                extra,
            )
        };
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({
                "pg-1": pg_connection("Shop", pgpass_model(merge_into(
                    tunnel(json!({ "keyPath": key.to_str().unwrap() })),
                    json!({ "save-password": false }),
                ))),
                "pg-2": pg_connection("Billing", pgpass_model(json!({
                    "configuration": { "database": "billing", "port": "5432" }
                }))),
                "pg-3": pg_connection("NoUser", pgpass_model(json!({})))
            })),
            Some(&json!({
                "pg-1": {
                    "#connection": { "user": "reader" },
                    "network/ssh_tunnel": { "user": "ssh-user" }
                },
                "pg-2": { "#connection": { "user": "reader" } }
            })),
        );
        let discovery = discover_pgpass(Some(home.path()), &pgpass, None);
        let by_name = |name: &str| {
            discovery
                .connections
                .iter()
                .find(|entry| entry.name == name)
                .unwrap()
                .clone()
        };
        let shop = by_name("Shop");
        assert_eq!(shop.status, ConnectionStatus::Available, "{shop:?}");
        let params = shop.params.unwrap();
        assert_eq!(params.user, "reader");
        assert_eq!(params.password.expose(), r"p:gp\ss");
        assert_eq!(params.tunnel.unwrap().hops[0].host, "bastion.example");
        assert_eq!(
            by_name("Billing").status,
            ConnectionStatus::Unavailable {
                reason: "no matching entry in the pgpass file".to_string()
            }
        );
        let no_user = by_name("NoUser");
        assert!(matches!(
            no_user.status,
            ConnectionStatus::Unavailable { .. }
        ));
        assert!(reason(&no_user.status).contains("user"));

        set_mode(&pgpass, 0o644);
        if cfg!(unix) {
            assert_eq!(
                by_name_in(&discover_pgpass(Some(home.path()), &pgpass, None), "Shop").status,
                ConnectionStatus::Unavailable {
                    reason: "pgpass file permissions are too open; run chmod 600 on it".to_string()
                }
            );
        }

        let absent = home.path().join("absent-pgpass");
        assert_eq!(
            by_name_in(&discover_pgpass(Some(home.path()), &absent, None), "Shop").status,
            ConnectionStatus::Unavailable {
                reason: "no matching entry in the pgpass file".to_string()
            }
        );
    }

    #[test]
    fn pgpass_est_cherche_avec_la_base_demandee_et_non_celle_de_dbeaver() {
        let home = TempDir::new().unwrap();
        let pgpass = home.path().join("pgpass-database-test");
        fs::write(
            &pgpass,
            "db.internal.example:5433:billing:reader:billing-pw\ndb.internal.example:5433:shop:reader:shop-pw\n",
        )
        .unwrap();
        set_mode(&pgpass, 0o600);
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({
                "pg-1": pg_connection("Shop", json!({
                    "configuration": { "auth-model": "postgres_pgpass" }
                }))
            })),
            Some(&json!({ "pg-1": { "#connection": { "user": "reader" } } })),
        );
        let default =
            resolve_connection(&discover_pgpass(Some(home.path()), &pgpass, None), "Shop").unwrap();
        assert_eq!(default.database, "shop");
        assert_eq!(default.password.expose(), "shop-pw");

        let billing = resolve_connection(
            &discover_pgpass(Some(home.path()), &pgpass, Some("billing")),
            "Shop",
        )
        .unwrap();
        assert_eq!(billing.database, "billing");
        assert_eq!(billing.password.expose(), "billing-pw");

        let unknown = resolve_connection(
            &discover_pgpass(Some(home.path()), &pgpass, Some("absent")),
            "Shop",
        );
        assert!(matches!(unknown, Err(ConnectionError::Unavailable { .. })));
    }

    #[cfg(unix)]
    fn set_mode(path: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    #[cfg(not(unix))]
    fn set_mode(_path: &Path, _mode: u32) {}

    fn by_name_in(discovery: &Discovery, name: &str) -> ConnectionEntry {
        discovery
            .connections
            .iter()
            .find(|entry| entry.name == name)
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn la_liste_des_connexions_masque_les_details_du_tunnel() {
        let home = TempDir::new().unwrap();
        let key = key_file(&home);
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({
                "pg-1": pg_connection("Shop", tunnel(json!({ "keyPath": key.to_str().unwrap() }))),
                "pg-2": pg_connection("Billing", tunnel(json!({ "authType": "PASSWORD" })))
            })),
            Some(&json!({
                "pg-1": {
                    "#connection": { "user": "canary-db-user", "password": "canary-db-pw" },
                    "network/ssh_tunnel": { "user": "canary-ssh-user", "password": "canary-passphrase" }
                },
                "pg-2": {
                    "#connection": { "user": "canary-db-user", "password": "canary-db-pw" },
                    "network/ssh_tunnel": { "user": "canary-ssh-user", "password": "canary-ssh-pw" }
                }
            })),
        );
        let result = list_connections(&config(home.path())).await;
        let text = serde_json::to_string(&result).unwrap();
        for forbidden in [
            "bastion.example",
            "2228",
            "canary-ssh-user",
            "canary-passphrase",
            "canary-ssh-pw",
            "canary-db-user",
            "canary-db-pw",
            "id_bastion",
            "db.internal.example",
        ] {
            assert!(!text.contains(forbidden), "{forbidden} leaked in {text}");
        }
        let body: Value = serde_json::from_str(
            result.content[0]
                .as_text()
                .map(|text| text.text.as_str())
                .unwrap(),
        )
        .unwrap();
        let connections = body["connections"].as_array().unwrap();
        assert_eq!(connections.len(), 2);
        for connection in connections {
            assert_eq!(connection["status"], "available", "{connection}");
        }
    }

    #[tokio::test]
    async fn la_liste_des_connexions_signale_la_verification_d_hote_contournee() {
        let home = TempDir::new().unwrap();
        write_project(
            home.path(),
            "General",
            "",
            &sources(json!({
                "pg-1": pg_connection("Shop", tunnel(json!({
                    "authType": "AGENT",
                    "bypassHostVerification": true
                }))),
                "pg-2": pg_connection("Billing", tunnel(json!({ "authType": "AGENT" }))),
                "pg-3": pg_connection("Direct", json!({}))
            })),
            Some(&json!({
                "pg-1": tunnel_creds("pg-1", json!({ "user": "ssh-user" }))["pg-1"],
                "pg-2": tunnel_creds("pg-2", json!({ "user": "ssh-user" }))["pg-2"],
                "pg-3": creds("pg-3", "reader", "db-pw")["pg-3"]
            })),
        );
        let result = list_connections(&config(home.path())).await;
        let body: Value = serde_json::from_str(
            result.content[0]
                .as_text()
                .map(|text| text.text.as_str())
                .unwrap(),
        )
        .unwrap();
        let connections = body["connections"].as_array().unwrap();
        let named = |name: &str| connections.iter().find(|c| c["name"] == name).unwrap();
        assert_eq!(named("Shop")["status"], "available", "{body}");
        assert_eq!(named("Shop")["ssh_host_key_check"], "bypassed", "{body}");
        assert!(
            named("Billing").get("ssh_host_key_check").is_none(),
            "{body}"
        );
        assert!(
            named("Direct").get("ssh_host_key_check").is_none(),
            "{body}"
        );
    }
}
