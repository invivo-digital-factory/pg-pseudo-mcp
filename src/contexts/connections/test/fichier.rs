use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tempfile::TempDir;

use super::dbeaver::PGPASS_ENV;
use crate::common::AppConfig;
use crate::contexts::connections::domain::types::{
    ConnectionEntry, ConnectionSource, ConnectionStatus, Discovery, SslMode,
};
use crate::contexts::connections::infrastructure::data_sources::azure::{AzureCli, SystemClock};
use crate::contexts::connections::infrastructure::data_sources::openssh::{
    LocalEnvironment, OpenSshResolver, SshCommand,
};
use crate::contexts::connections::infrastructure::repositories::{
    CachedAzureSignInTokens, FileConnectionCatalog,
};
use crate::contexts::connections::injector::ConnectionsContext;

const AZURE_HOST: &str = "shop.postgres.database.azure.com";
const WORKING_AZ: &str = r#"echo '{"accessToken":"entra-token-1","expires_on":4102444800}'"#;
const LOGGED_OUT_AZ: &str = "echo \"ERROR: Please run 'az login'\" >&2\nexit 1";

struct Setup {
    dir: TempDir,
}

impl Setup {
    fn new(az_body: &str, pgpass_line: &str) -> Self {
        let dir = TempDir::new().unwrap();
        let az = dir.path().join("az");
        fs::write(&az, format!("#!/bin/sh\n{az_body}\n")).unwrap();
        fs::set_permissions(&az, fs::Permissions::from_mode(0o755)).unwrap();
        let pgpass = dir.path().join("pgpass");
        fs::write(&pgpass, pgpass_line).unwrap();
        fs::set_permissions(&pgpass, fs::Permissions::from_mode(0o600)).unwrap();
        unsafe { std::env::set_var("PGPASSFILE", &pgpass) };
        Self { dir }
    }

    fn file(&self) -> PathBuf {
        self.dir.path().join("connections.toml")
    }

    fn write(&self, contents: &str) {
        fs::write(self.file(), contents).unwrap();
    }

    fn marker(&self) -> PathBuf {
        self.dir.path().join("az-ran")
    }

    fn catalog_with(&self, ssh: Option<PathBuf>) -> FileConnectionCatalog {
        let tokens = Arc::new(CachedAzureSignInTokens::new(
            Arc::new(AzureCli::new(Some(self.dir.path().join("az")))),
            Arc::new(SystemClock),
        ));
        let resolver = Arc::new(OpenSshResolver::new(
            Arc::new(SshCommand::new(ssh, None)),
            LocalEnvironment::default(),
        ));
        FileConnectionCatalog::new(Some(self.file()), tokens, resolver)
    }

    fn discover(&self) -> Discovery {
        self.catalog_with(None).load(None).discovery
    }
}

fn entry(discovery: &Discovery, name: &str) -> ConnectionEntry {
    discovery
        .connections
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("{name} is not listed"))
        .clone()
}

fn azure_entry(host: &str) -> String {
    format!(
        "[[connection]]\nname = \"shop\"\nhost = \"{host}\"\ndatabase = \"shop\"\nuser = \"dev@corp.example\"\ntype = \"dev\"\nauth = \"azure\"\n"
    )
}

fn reason_of(entry: &ConnectionEntry) -> String {
    match &entry.status {
        ConnectionStatus::Unavailable { reason } | ConnectionStatus::Unsupported { reason } => {
            reason.clone()
        }
        ConnectionStatus::Available => panic!("the connection is available"),
    }
}

#[test]
fn une_connexion_azure_sans_az_connecte_ni_entree_pgpass_demande_az_login() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new(LOGGED_OUT_AZ, "# empty\n");
    setup.write(&azure_entry(AZURE_HOST));
    let shop = entry(&setup.discover(), "shop");
    assert!(reason_of(&shop).contains("az login"), "{shop:?}");
    assert_eq!(shop.source, ConnectionSource::File);
}

#[test]
fn une_connexion_azure_declaree_sur_un_hote_hors_azure_ne_lance_jamais_az_et_n_envoie_aucun_jeton()
{
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let marker = TempDir::new().unwrap();
    let body = format!("touch {}/az-ran\n{WORKING_AZ}", marker.path().display());
    let setup = Setup::new(&body, "# empty\n");
    setup.write(&azure_entry("db.example.com"));
    let shop = entry(&setup.discover(), "shop");
    let reason = reason_of(&shop);
    assert!(
        reason.contains("only sent to Azure PostgreSQL hosts"),
        "{reason}"
    );
    assert!(shop.params.is_none());
    assert!(!marker.path().join("az-ran").exists());
    let looks_like_azure = "shop.postgres.database.azure.com.evil.example";
    setup.write(&azure_entry(looks_like_azure));
    let shop = entry(&setup.discover(), "shop");
    assert!(shop.params.is_none());
    assert!(!marker.path().join("az-ran").exists());
    assert!(!setup.marker().exists());
}

#[test]
fn une_connexion_azure_sur_un_hote_azure_ou_prive_prend_le_jeton_et_impose_tls() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new(WORKING_AZ, "# empty\n");
    for host in [AZURE_HOST, "shop.privatelink.postgres.database.azure.com"] {
        setup.write(&azure_entry(host));
        let params = entry(&setup.discover(), "shop").params.expect("available");
        assert_eq!(params.password.expose(), "entra-token-1");
        assert!(params.password.is_azure_token());
        assert_eq!(params.sslmode, SslMode::Require, "{host}");
    }
}

#[test]
fn en_mode_pgpass_le_mot_de_passe_est_apparie_sur_l_hote_ecrit_dans_le_fichier() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new(
        "exit 1",
        "db.internal.example:5432:app:reader:file-secret\n",
    );
    setup.write(
        "[[connection]]\nname = \"matching\"\nhost = \"db.internal.example\"\ndatabase = \"app\"\nuser = \"reader\"\ntype = \"dev\"\nauth = \"pgpass\"\n\n[[connection]]\nname = \"other\"\nhost = \"other.internal.example\"\ndatabase = \"app\"\nuser = \"reader\"\ntype = \"dev\"\nauth = \"pgpass\"\n",
    );
    let discovery = setup.discover();
    let matching = entry(&discovery, "matching");
    assert_eq!(matching.status, ConnectionStatus::Available, "{matching:?}");
    let params = matching.params.unwrap();
    assert_eq!(params.password.expose(), "file-secret");
    assert_eq!(params.sslmode, SslMode::Prefer);
    assert!(reason_of(&entry(&discovery, "other")).contains("pgpass"));
}

#[test]
fn sans_ssh_dans_le_path_seules_les_connexions_a_alias_sont_indisponibles() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new("exit 1", "# empty\n");
    setup.write(
        "[[connection]]\nname = \"tunneled\"\nhost = \"db\"\nuser = \"u\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"x\"]\nssh_alias = \"bastion\"\n\n[[connection]]\nname = \"direct\"\nhost = \"db\"\nuser = \"u\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"x\"]\n",
    );
    let discovery = setup.discover();
    assert!(reason_of(&entry(&discovery, "tunneled")).contains("ssh"));
    assert_eq!(
        entry(&discovery, "direct").status,
        ConnectionStatus::Available
    );
}

#[test]
fn les_modifications_du_fichier_sont_prises_au_prochain_appel_sans_redemarrage() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new("exit 1", "# empty\n");
    let command_entry = |name: &str| {
        format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"db\"\nuser = \"u\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"x\"]\n"
        )
    };
    let catalog = setup.catalog_with(None);
    setup.write(&command_entry("first"));
    assert_eq!(catalog.load(None).discovery.connections.len(), 1);
    setup.write(&format!(
        "{}{}",
        command_entry("first"),
        command_entry("second")
    ));
    let names: Vec<String> = catalog
        .load(None)
        .discovery
        .connections
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(names, vec!["first", "second"]);
}

#[test]
fn un_fichier_nomme_par_la_variable_et_absent_est_signale_avec_son_chemin() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new("exit 1", "# empty\n");
    let load = setup.catalog_with(None).load(None);
    let hint = load.discovery.hint.expect("hint");
    assert!(hint.contains("does not exist"), "{hint}");
    assert!(hint.contains(&setup.file().display().to_string()), "{hint}");
    assert!(load.discovery.connections.is_empty());
    assert!(load.read_dbeaver);
}

#[test]
fn un_fichier_illisible_comme_toml_est_signale_avec_sa_ligne_et_laisse_dbeaver_actif() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = Setup::new("exit 1", "# empty\n");
    setup.write("[[connection]]\nname = \"a\" \"b\"\n");
    let load = setup.catalog_with(None).load(None);
    let hint = load.discovery.hint.expect("hint");
    assert!(hint.contains("line 2"), "{hint}");
    assert!(load.read_dbeaver);
}

fn context_for(path: &Path) -> ConnectionsContext {
    ConnectionsContext::new(&AppConfig {
        connections_file: Some(path.to_path_buf()),
        ..AppConfig::default()
    })
}

#[tokio::test]
#[ignore = "needs the docker compose database"]
async fn l_argument_database_atteint_une_autre_base_du_serveur_sur_une_connexion_du_fichier() {
    use std::str::FromStr;

    use crate::contexts::connections::domain::types::{Cell, ExecutionLimits};
    use crate::contexts::connections::test::database::database_url;

    let config = tokio_postgres::Config::from_str(&database_url()).expect("database url");
    let host = match config.get_hosts().first() {
        Some(tokio_postgres::config::Host::Tcp(host)) => host.clone(),
        _ => "localhost".to_string(),
    };
    let port = config.get_ports().first().copied().unwrap_or(5432);
    let user = config.get_user().unwrap_or("pgpseudo").to_string();
    let password = String::from_utf8_lossy(config.get_password().expect("password")).into_owned();
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("connections.toml");
    fs::write(
        &file,
        format!(
            "[[connection]]\nname = \"local\"\nhost = \"{host}\"\nport = {port}\ndatabase = \"{}\"\nuser = \"{user}\"\nsslmode = \"disable\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"{password}\"]\n",
            config.get_dbname().unwrap_or("pgpseudo")
        ),
    )
    .unwrap();
    let context = context_for(&file);
    let limits = ExecutionLimits {
        max_rows: 10,
        timeout_ms: 10_000,
    };
    let default = context
        .run_read_only_statement
        .execute("local", None, "SELECT current_database()::text", limits)
        .await
        .expect("default database");
    let other = context
        .run_read_only_statement
        .execute(
            "local",
            Some("postgres"),
            "SELECT current_database()::text",
            limits,
        )
        .await
        .expect("other database");
    assert_ne!(default.rows[0][0], other.rows[0][0]);
    assert_eq!(other.rows[0][0], Cell::Text("postgres".into()));
}
