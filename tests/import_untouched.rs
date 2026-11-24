use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use scarif_database_mcp::contexts::connections::infrastructure::data_sources::dbeaver::credentials::encrypt_for_tests;
use serde_json::json;

const PGPASS_CONTENT: &[u8] = b"db.internal.example:5432:shop:reader:kept-secret-9921\n";
const SSH_CONFIG_CONTENT: &[u8] = b"Host kept-alias\n  HostName kept.internal.example\n";

mod e2e {
    use super::*;

    #[test]
    fn l_import_laisse_le_fichier_pgpass_et_la_configuration_ssh_octet_pour_octet() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("dbeaver");
        let sources_dir = workspace
            .join("workspace6")
            .join("General")
            .join(".dbeaver");
        std::fs::create_dir_all(&sources_dir).unwrap();
        std::fs::write(
            sources_dir.join("data-sources.json"),
            json!({
                "folders": {},
                "connections": {
                    "postgres-jdbc-one": {
                        "provider": "postgresql",
                        "driver": "postgres-jdbc",
                        "name": "tunneled",
                        "save-password": true,
                        "configuration": {
                            "host": "db.internal.example",
                            "port": "5432",
                            "database": "shop",
                            "type": "dev",
                            "auth-model": "native",
                            "provider-properties": { "sslmode": "disable" },
                            "handlers": {
                                "ssh_tunnel": {
                                    "type": "TUNNEL",
                                    "enabled": true,
                                    "properties": {
                                        "authType": "PUBLIC_KEY",
                                        "host": "bastion.corp.example",
                                        "port": 22,
                                        "jumpServer.count": 0,
                                        "keyPath": "bastion-key"
                                    }
                                }
                            }
                        }
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            sources_dir.join("credentials-config.json"),
            encrypt_for_tests(
                &json!({ "postgres-jdbc-one": {
                    "#connection": { "user": "reader", "password": "unused" },
                    "network/ssh_tunnel": { "user": "jump" }
                } })
                .to_string(),
                [7u8; 16],
            ),
        )
        .unwrap();
        let pgpass = dir.path().join("pgpass");
        std::fs::write(&pgpass, PGPASS_CONTENT).unwrap();
        std::fs::set_permissions(&pgpass, std::fs::Permissions::from_mode(0o600)).unwrap();
        let ssh_config = dir.path().join("ssh_config");
        std::fs::write(&ssh_config, SSH_CONFIG_CONTENT).unwrap();
        let connections = dir.path().join("config").join("connections.toml");

        let output = Command::new(env!("CARGO_BIN_EXE_scarif-database-mcp"))
            .args(["connections", "import-dbeaver"])
            .env("HOME", dir.path())
            .env("SCARIF_DATA_DIR", dir.path().join("data"))
            .env("SCARIF_DBEAVER_HOME", &workspace)
            .env("SCARIF_CONNECTIONS_FILE", &connections)
            .env("PGPASSFILE", &pgpass)
            .env("SCARIF_SSH_CONFIG", &ssh_config)
            .output()
            .expect("binary runs");

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(connections.exists());
        let summary = String::from_utf8_lossy(&output.stdout);
        assert!(summary.contains("Host bastion"), "{summary}");
        assert_eq!(std::fs::read(&pgpass).unwrap(), PGPASS_CONTENT);
        assert_eq!(std::fs::read(&ssh_config).unwrap(), SSH_CONFIG_CONTENT);
    }
}
