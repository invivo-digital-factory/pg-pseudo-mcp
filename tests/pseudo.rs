use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use scarif_database_mcp::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use scarif_database_mcp::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use scarif_database_mcp::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;

fn open(dir: &Path) -> Pseudonymizer {
    open_pseudonymizer(dir, GeneratorConfig::default()).expect("open pseudonymizer")
}

mod e2e {
    use super::*;

    #[test]
    fn la_commande_de_purge_du_mapping_demande_une_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        open(dir.path()).pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr");
        let binary = env!("CARGO_BIN_EXE_scarif-database-mcp");

        let mut refused = Command::new(binary)
            .args(["mapping", "purge"])
            .env("SCARIF_DATA_DIR", dir.path())
            .env(
                "SCARIF_CONNECTIONS_FILE",
                dir.path().join("absent-connections.toml"),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        refused.stdin.take().unwrap().write_all(b"no\n").unwrap();
        let refused = refused.wait_with_output().unwrap();
        assert!(!refused.status.success());
        assert!(dir.path().join("secret").exists());

        let mut confirmed = Command::new(binary)
            .args(["mapping", "purge"])
            .env("SCARIF_DATA_DIR", dir.path())
            .env(
                "SCARIF_CONNECTIONS_FILE",
                dir.path().join("absent-connections.toml"),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        confirmed.stdin.take().unwrap().write_all(b"yes\n").unwrap();
        let confirmed = confirmed.wait_with_output().unwrap();
        assert!(confirmed.status.success());
        let stderr = String::from_utf8_lossy(&confirmed.stderr);
        assert!(stderr.contains("mapping.sqlite"), "{stderr}");
        assert!(!dir.path().join("secret").exists());
        assert!(!dir.path().join("mapping.sqlite").exists());

        open(dir.path());
        let forced = Command::new(binary)
            .args(["mapping", "purge", "--yes"])
            .env("SCARIF_DATA_DIR", dir.path())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(forced.status.success());
        assert!(!dir.path().join("secret").exists());
    }
}
