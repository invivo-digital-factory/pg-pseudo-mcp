use std::os::unix::fs::PermissionsExt;

use super::*;
use crate::contexts::connections::infrastructure::data_sources::azure::{AzureCli, SystemClock};
use crate::contexts::connections::infrastructure::repositories::CachedAzureSignInTokens;

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
    let discovery = discover_with(Some(setup.home.path()), tokens(setup).as_ref());
    by_name(&discovery, "Shop")
}

fn use_pgpass(setup: &Setup) {
    unsafe { std::env::set_var("PGPASSFILE", &setup.pgpass) };
}

#[test]
fn un_serveur_azure_en_pgpass_prend_le_jeton_du_cli_meme_si_pgpass_a_une_entree() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        Some(MATCHING_PGPASS),
        WORKING_AZ,
    );
    use_pgpass(&setup);
    let entry = shop(&setup);
    assert_eq!(entry.status, ConnectionStatus::Available, "{entry:?}");
    let params = entry.params.unwrap();
    assert_eq!(params.password.expose(), "entra-token-1");
    assert!(params.password_is_azure_token);
}

#[test]
fn un_serveur_azure_en_pgpass_sans_entree_pgpass_utilise_le_jeton_du_cli() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(AZURE_HOST, pgpass_model(), None, WORKING_AZ);
    use_pgpass(&setup);
    let params = shop(&setup).params.expect("available");
    assert_eq!(params.password.expose(), "entra-token-1");
}

#[test]
fn un_az_en_echec_retombe_sur_l_entree_pgpass_quand_elle_correspond() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        Some(MATCHING_PGPASS),
        LOGGED_OUT_AZ,
    );
    use_pgpass(&setup);
    let entry = shop(&setup);
    assert_eq!(entry.status, ConnectionStatus::Available, "{entry:?}");
    let params = entry.params.unwrap();
    assert_eq!(params.password.expose(), "pgpass-secret");
    assert!(!params.password_is_azure_token);
}

#[test]
fn list_connections_demande_az_login_quand_az_echoue_et_que_pgpass_ne_correspond_pas() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let home_dir = std::env::var("HOME").unwrap();
    let failing = format!(
        "echo \"ERROR: cannot read {home_dir}/.azure/msal_token_cache; run az login\" >&2\necho 'second-line-canary' >&2\nexit 1"
    );
    let setup = setup(AZURE_HOST, pgpass_model(), None, &failing);
    use_pgpass(&setup);
    let catalog =
        DbeaverConnectionCatalog::new(Some(setup.home.path().to_path_buf()), tokens(&setup));
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
    assert!(reason.contains("~/.azure"), "{reason}");
    assert!(!text.contains("second-line-canary"), "{text}");
    assert!(!text.contains("shop.privatelink"), "{text}");
}

#[test]
fn un_az_absent_donne_une_raison_qui_demande_de_l_installer_puis_de_se_connecter() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(AZURE_HOST, pgpass_model(), None, WORKING_AZ);
    use_pgpass(&setup);
    let discovery = discover_with(Some(setup.home.path()), &NoTokens);
    let entry = by_name(&discovery, "Shop");
    assert!(reason(&entry.status).contains("not found on PATH"));
    assert!(reason(&entry.status).contains("az login"));
}

#[test]
fn un_hote_hors_azure_en_pgpass_n_appelle_jamais_az() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(
        "db.internal.example",
        pgpass_model(),
        Some("db.internal.example:5433:shop:dev@corp.example:plain-secret\n"),
        "touch \"$0.called\"\nexit 1",
    );
    use_pgpass(&setup);
    let params = shop(&setup).params.expect("available");
    assert_eq!(params.password.expose(), "plain-secret");
    assert!(!params.password_is_azure_token);
    assert!(!setup.bin.path().join("az.called").exists());
}

#[test]
fn un_serveur_azure_en_authentification_native_n_appelle_jamais_az() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(
        AZURE_HOST,
        json!({ "configuration": { "password": "native-secret" } }),
        None,
        "touch \"$0.called\"\nexit 1",
    );
    let params = shop(&setup).params.expect("available");
    assert_eq!(params.password.expose(), "native-secret");
    assert!(!params.password_is_azure_token);
    assert!(!setup.bin.path().join("az.called").exists());
}

#[test]
fn un_seul_appel_a_az_sert_toutes_les_connexions_azure_d_une_decouverte() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(
        AZURE_HOST,
        pgpass_model(),
        None,
        "echo x >> \"$0.count\"\nexit 1",
    );
    use_pgpass(&setup);
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
    let discovery = discover_with(Some(setup.home.path()), tokens(&setup).as_ref());
    assert_eq!(discovery.connections.len(), 2);
    let count = fs::read_to_string(setup.bin.path().join("az.count")).unwrap();
    assert_eq!(count.lines().count(), 1);
}

fn azure_pgpass_sslmode(extra: Value) -> SslMode {
    let mut model = pgpass_model();
    merge(&mut model, extra);
    let setup = setup(AZURE_HOST, model, None, WORKING_AZ);
    use_pgpass(&setup);
    let params = shop(&setup).params.expect("available");
    assert!(params.password_is_azure_token);
    params.sslmode
}

#[test]
fn un_jeton_azure_sans_sslmode_impose_tls() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(azure_pgpass_sslmode(json!({})), SslMode::Require);
}

#[test]
fn un_jeton_azure_avec_sslmode_disable_allow_ou_prefer_impose_tls() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    for mode in ["disable", "allow", "prefer"] {
        let extra = json!({ "configuration": { "provider-properties": { "sslmode": mode } } });
        assert_eq!(azure_pgpass_sslmode(extra), SslMode::Require, "{mode}");
    }
}

#[test]
fn un_jeton_azure_garde_une_verification_plus_stricte_que_require() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let extra = json!({ "configuration": { "provider-properties": { "sslmode": "verify-full" } } });
    assert_eq!(azure_pgpass_sslmode(extra), SslMode::VerifyFull);
}

#[test]
fn un_mot_de_passe_pgpass_hors_azure_garde_prefer() {
    let _env = PGPASS_ENV.lock().unwrap_or_else(|e| e.into_inner());
    let setup = setup(
        "db.internal.example",
        pgpass_model(),
        Some("db.internal.example:5433:shop:dev@corp.example:plain-secret\n"),
        WORKING_AZ,
    );
    use_pgpass(&setup);
    assert_eq!(shop(&setup).params.unwrap().sslmode, SslMode::Prefer);
}
