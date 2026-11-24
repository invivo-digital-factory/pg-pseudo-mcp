use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::common::AppConfig;
use crate::contexts::connections::infrastructure::data_sources::azure::{AzureCli, SystemClock};
use crate::contexts::connections::infrastructure::repositories::CachedAzureSignInTokens;
use crate::contexts::connections::injector::ConnectionsContext;

const AZURE_HOST: &str = "shop.privatelink.postgres.database.azure.com";
const WORKING_AZ: &str = r#"echo '{"accessToken":"entra-token-1","expires_on":4102444800}'"#;
const LOGGED_OUT_AZ: &str = "echo \"ERROR: Please run 'az login' to setup account.\" >&2\nexit 1";
const MATCHING_PGPASS: &str =
    "shop.privatelink.postgres.database.azure.com:5433:shop:dev@corp.example:pgpass-secret\n";

struct Setup {
    home: TempDir,
    bin: TempDir,
    pgpass: PathBuf,
}

fn connection_json(host: &str, name: &str, extra: Value) -> Value {
    let mut connection = pg_connection(
        name,
        json!({ "configuration": { "host": host, "user": "dev@corp.example" } }),
    );
    merge(&mut connection, extra);
    connection
}

fn setup(host: &str, extra: Value, pgpass_line: Option<&str>, az_body: &str) -> Setup {
    let home = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    let az = bin.path().join("az");
    fs::write(&az, format!("#!/bin/sh\n{az_body}\n")).unwrap();
    fs::set_permissions(&az, fs::Permissions::from_mode(0o755)).unwrap();
    let pgpass = home.path().join("pgpass-azure-test");
    fs::write(&pgpass, pgpass_line.unwrap_or("# empty\n")).unwrap();
    fs::set_permissions(&pgpass, fs::Permissions::from_mode(0o600)).unwrap();
    write_project(
        home.path(),
        "General",
        "",
        &sources(json!({ "pg-1": connection_json(host, "Shop", extra) })),
        None,
    );
    Setup { home, bin, pgpass }
}

fn pgpass_model() -> Value {
    json!({ "configuration": { "auth-model": "postgres_pgpass" } })
}

fn tokens(setup: &Setup) -> Arc<CachedAzureSignInTokens> {
    Arc::new(CachedAzureSignInTokens::new(
        Arc::new(AzureCli::new(Some(setup.bin.path().join("az")))),
        Arc::new(SystemClock),
    ))
}

fn by_name(discovery: &Discovery, name: &str) -> ConnectionEntry {
    discovery
        .connections
        .iter()
        .find(|entry| entry.name == name)
        .unwrap()
        .clone()
}

fn shop(setup: &Setup) -> ConnectionEntry {
    let discovery = discover_with(
        Some(setup.home.path()),
        tokens(setup).as_ref(),
        Some(&setup.pgpass),
    );
    by_name(&discovery, "Shop")
}

#[test]
fn un_serveur_azure_en_pgpass_prend_le_jeton_du_cli_meme_si_pgpass_a_une_entree() {
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        Some(MATCHING_PGPASS),
        WORKING_AZ,
    );
    let entry = shop(&setup);
    assert_eq!(entry.status, ConnectionStatus::Available, "{entry:?}");
    let params = entry.params.unwrap();
    assert_eq!(params.password.expose(), "entra-token-1");
    assert!(params.password.is_azure_token());
}

#[test]
fn un_serveur_azure_en_pgpass_sans_entree_pgpass_utilise_le_jeton_du_cli() {
    let setup = setup(AZURE_HOST, pgpass_model(), None, WORKING_AZ);
    let params = shop(&setup).params.expect("available");
    assert_eq!(params.password.expose(), "entra-token-1");
}

#[test]
fn un_az_en_echec_retombe_sur_l_entree_pgpass_quand_elle_correspond() {
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        Some(MATCHING_PGPASS),
        LOGGED_OUT_AZ,
    );
    let entry = shop(&setup);
    assert_eq!(entry.status, ConnectionStatus::Available, "{entry:?}");
    let params = entry.params.unwrap();
    assert_eq!(params.password.expose(), "pgpass-secret");
    assert!(!params.password.is_azure_token());
}

#[test]
fn list_connections_demande_az_login_quand_az_echoue_et_que_pgpass_ne_correspond_pas() {
    let home_dir = std::env::var("HOME").unwrap();
    let failing = format!(
        "echo \"ERROR: cannot read {home_dir}/.azure/msal_token_cache; run az login\" >&2\necho 'second-line-canary' >&2\nexit 1"
    );
    let setup = setup(AZURE_HOST, pgpass_model(), None, &failing);
    let catalog = DbeaverConnectionCatalog::new(
        Some(setup.home.path().to_path_buf()),
        tokens(&setup),
        Some(setup.pgpass.clone()),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let result = runtime.block_on(presentation::list_connections(&ListConnections::new(
        Arc::new(catalog),
    )));
    let text = result.content[0]
        .as_text()
        .map(|text| text.text.clone())
        .unwrap();
    let body: Value = serde_json::from_str(&text).unwrap();
    let connection = &body["connections"][0];
    assert_eq!(connection["status"], "unavailable", "{body}");
    let reason = connection["reason"].as_str().unwrap();
    assert!(reason.contains("az login"), "{reason}");
    assert!(!reason.contains(&home_dir), "{reason}");
    assert!(!reason.contains("msal_token_cache"), "{reason}");
    assert!(!reason.contains("cannot read"), "{reason}");
    assert!(!text.contains("second-line-canary"), "{text}");
    assert!(!text.contains("shop.privatelink"), "{text}");
}

#[test]
fn un_az_absent_donne_une_raison_qui_demande_de_l_installer_puis_de_se_connecter() {
    let setup = setup(AZURE_HOST, pgpass_model(), None, WORKING_AZ);
    let discovery = discover_with(Some(setup.home.path()), &NoTokens, Some(&setup.pgpass));
    let entry = by_name(&discovery, "Shop");
    assert!(reason(&entry.status).contains("not found on PATH"));
    assert!(reason(&entry.status).contains("az login"));
}

#[test]
fn un_hote_hors_azure_en_pgpass_n_appelle_jamais_az() {
    let setup = setup(
        "db.internal.example",
        pgpass_model(),
        Some("db.internal.example:5433:shop:dev@corp.example:plain-secret\n"),
        "touch \"$0.called\"\nexit 1",
    );
    let params = shop(&setup).params.expect("available");
    assert_eq!(params.password.expose(), "plain-secret");
    assert!(!params.password.is_azure_token());
    assert!(!setup.bin.path().join("az.called").exists());
}

#[test]
fn un_serveur_azure_en_authentification_native_n_appelle_jamais_az() {
    let setup = setup(
        AZURE_HOST,
        json!({ "configuration": { "password": "native-secret" } }),
        None,
        "touch \"$0.called\"\nexit 1",
    );
    let params = shop(&setup).params.expect("available");
    assert_eq!(params.password.expose(), "native-secret");
    assert!(!params.password.is_azure_token());
    assert!(!setup.bin.path().join("az.called").exists());
}

#[test]
fn un_seul_appel_a_az_sert_toutes_les_connexions_azure_d_une_decouverte() {
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        None,
        "echo x >> \"$0.count\"\nexit 1",
    );
    write_project(
        setup.home.path(),
        "General",
        "",
        &sources(json!({
            "pg-1": connection_json(AZURE_HOST, "Shop", pgpass_model()),
            "pg-2": connection_json(AZURE_HOST, "Billing", pgpass_model())
        })),
        None,
    );
    let discovery = discover_with(
        Some(setup.home.path()),
        tokens(&setup).as_ref(),
        Some(&setup.pgpass),
    );
    assert_eq!(discovery.connections.len(), 2);
    let count = fs::read_to_string(setup.bin.path().join("az.count")).unwrap();
    assert_eq!(count.lines().count(), 1);
}

fn azure_pgpass_sslmode(extra: Value) -> SslMode {
    let mut model = pgpass_model();
    merge(&mut model, extra);
    let setup = setup(AZURE_HOST, model, None, WORKING_AZ);
    let params = shop(&setup).params.expect("available");
    assert!(params.password.is_azure_token());
    params.sslmode
}

#[test]
fn un_jeton_azure_sans_sslmode_impose_tls() {
    assert_eq!(azure_pgpass_sslmode(json!({})), SslMode::Require);
}

#[test]
fn un_jeton_azure_avec_sslmode_disable_allow_ou_prefer_impose_tls() {
    for mode in ["disable", "allow", "prefer"] {
        let extra = json!({ "configuration": { "provider-properties": { "sslmode": mode } } });
        assert_eq!(azure_pgpass_sslmode(extra), SslMode::Require, "{mode}");
    }
}

#[test]
fn un_jeton_azure_garde_une_verification_plus_stricte_que_require() {
    let extra = json!({ "configuration": { "provider-properties": { "sslmode": "verify-full" } } });
    assert_eq!(azure_pgpass_sslmode(extra), SslMode::VerifyFull);
}

#[test]
fn un_mot_de_passe_pgpass_hors_azure_garde_prefer() {
    let setup = setup(
        "db.internal.example",
        pgpass_model(),
        Some("db.internal.example:5433:shop:dev@corp.example:plain-secret\n"),
        WORKING_AZ,
    );
    assert_eq!(shop(&setup).params.unwrap().sslmode, SslMode::Prefer);
}

#[test]
fn deux_decouvertes_rapprochees_apres_un_echec_de_az_n_appellent_az_qu_une_fois() {
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        None,
        "echo x >> \"$0.count\"\nexit 1",
    );
    write_project(
        setup.home.path(),
        "General",
        "",
        &sources(json!({
            "pg-1": connection_json(AZURE_HOST, "Shop", pgpass_model()),
            "pg-2": connection_json("db.internal.example", "Plain", json!({
                "configuration": { "password": "native-secret" }
            }))
        })),
        None,
    );
    let tokens = tokens(&setup);
    let first = discover_with(
        Some(setup.home.path()),
        tokens.as_ref(),
        Some(&setup.pgpass),
    );
    let second = discover_with(
        Some(setup.home.path()),
        tokens.as_ref(),
        Some(&setup.pgpass),
    );
    assert_eq!(by_name(&first, "Plain").status, ConnectionStatus::Available);
    assert_eq!(
        by_name(&second, "Plain").status,
        ConnectionStatus::Available
    );
    let count = fs::read_to_string(setup.bin.path().join("az.count")).unwrap();
    assert_eq!(count.lines().count(), 1);
}

const SLOW_AZ: &str = "sleep 1\nexit 1";

fn mixed_workspace(setup: &Setup) {
    write_project(
        setup.home.path(),
        "General",
        "",
        &sources(json!({
            "pg-1": connection_json(AZURE_HOST, "Shop", pgpass_model()),
            "pg-2": connection_json("127.0.0.1", "Plain", json!({
                "configuration": { "password": "native-secret", "port": "1" }
            }))
        })),
        None,
    );
}

#[test]
fn list_connections_ne_bloque_pas_le_runtime_pendant_que_az_repond() {
    let setup = setup(AZURE_HOST, pgpass_model(), None, SLOW_AZ);
    mixed_workspace(&setup);
    let ssh_config = setup.home.path().join("ssh_config");
    fs::write(&ssh_config, "").unwrap();
    let context = ConnectionsContext::with_azure_command(
        &AppConfig {
            dbeaver_home: Some(setup.home.path().to_path_buf()),
            data_dir: None,
            ssh_known_hosts: None,
            ssh_config: Some(ssh_config),
            connections_file: Some(setup.home.path().join("absent-connections.toml")),
            columns_file: None,
            xdg_config_home: None,
            home_dir: None,
            data_home: None,
        },
        Some(setup.bin.path().join("az")),
        Some(setup.pgpass.clone()),
    );
    let use_case = &context.list_connections;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let stall = runtime.block_on(async {
        let ticks = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counter = ticks.clone();
        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        presentation::list_connections(use_case).await;
        ticker.abort();
        ticks.load(std::sync::atomic::Ordering::SeqCst)
    });
    assert!(stall >= 20, "ticks while az was running: {stall}");
}

#[test]
fn une_requete_sur_une_connexion_hors_azure_ne_bloque_pas_le_runtime_pendant_que_az_repond() {
    use crate::contexts::connections::domain::repositories::SessionPool;
    use crate::contexts::connections::domain::types::ExecutionLimits;
    use crate::contexts::connections::infrastructure::data_sources::password_command::ProcessPasswordCommand;
    use crate::contexts::connections::infrastructure::repositories::{
        CachedCommandPasswords, PostgresSessionPool,
    };

    let setup = setup(AZURE_HOST, pgpass_model(), None, SLOW_AZ);
    mixed_workspace(&setup);
    let tokens = tokens(&setup);
    let pool = PostgresSessionPool::new(
        Arc::new(DbeaverConnectionCatalog::new(
            Some(setup.home.path().to_path_buf()),
            tokens.clone(),
            Some(setup.pgpass.clone()),
        )),
        None,
        tokens,
        Arc::new(CachedCommandPasswords::new(Arc::new(
            ProcessPasswordCommand::new(),
        ))),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let ticks = runtime.block_on(async {
        let ticks = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counter = ticks.clone();
        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
        let limits = ExecutionLimits {
            max_rows: 1,
            timeout_ms: 1_000,
        };
        let _ = pool.run_read_only("Plain", None, "SELECT 1", limits).await;
        ticker.abort();
        ticks.load(std::sync::atomic::Ordering::SeqCst)
    });
    assert!(ticks >= 20, "ticks while az was running: {ticks}");
}
