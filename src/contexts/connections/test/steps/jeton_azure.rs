use cucumber::{given, then};
use serde_json::json;

use crate::connections_steps::support::{
    make_executable, postgres_connection, saved, write_connection_file, write_dbeaver,
};
use crate::connections_world::ConnectionsWorld;

fn azure_cli(world: &mut ConnectionsWorld, body: &str) {
    let dir = world.config_dir.path();
    let runs = dir.join("az-runs");
    let script = dir.join("az");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho run >> '{}'\n{body}\n", runs.display()),
    )
    .expect("az script");
    make_executable(&script);
    world.azure_cli = Some(script);
}

#[given(regex = r#"^un Azure CLI connecté qui délivre le jeton "([^"]+)"$"#)]
async fn signed_in_azure_cli(world: &mut ConnectionsWorld, token: String) {
    let answer = json!({ "accessToken": token, "expires_on": 4_102_444_800_u64 });
    azure_cli(world, &format!("echo '{answer}'"));
}

#[given(regex = r#"^un Azure CLI qui demande de se connecter avec le message "([^"]+)"$"#)]
async fn signed_out_azure_cli(world: &mut ConnectionsWorld, message: String) {
    azure_cli(world, &format!("echo '{message}' >&2\nexit 1"));
}

#[given(regex = r#"^un Azure CLI qui répond "([^"]+)"$"#)]
async fn unreadable_azure_cli(world: &mut ConnectionsWorld, answer: String) {
    azure_cli(world, &format!("echo '{answer}'"));
}

#[given(regex = r"^un Azure CLI absent du poste$")]
async fn missing_azure_cli(world: &mut ConnectionsWorld) {
    world.azure_cli = None;
}

fn azure_runs(world: &ConnectionsWorld) -> usize {
    std::fs::read_to_string(world.config_dir.path().join("az-runs"))
        .unwrap_or_default()
        .lines()
        .count()
}

#[then(regex = r"^l'Azure CLI n'a pas été lancé$")]
async fn azure_cli_not_run(world: &mut ConnectionsWorld) {
    assert_eq!(azure_runs(world), 0);
}

#[then(regex = r"^l'Azure CLI a été lancé (\d+) fois$")]
async fn azure_cli_ran(world: &mut ConnectionsWorld, count: usize) {
    assert_eq!(azure_runs(world), count);
}

fn azure_entry(name: &str, host: &str, sslmode: Option<&str>) -> String {
    let sslmode = sslmode
        .map(|mode| format!("sslmode = \"{mode}\"\n"))
        .unwrap_or_default();
    format!(
        "[[connection]]\nname = \"{name}\"\nhost = \"{host}\"\ndatabase = \"shop\"\nuser = \"alice@example.com\"\n{sslmode}type = \"dev\"\nauth = \"azure\"\n"
    )
}

#[given(regex = r#"^une connexion du fichier "([^"]+)" en mode azure vers l'hôte "([^"]+)"$"#)]
async fn azure_connection(world: &mut ConnectionsWorld, name: String, host: String) {
    write_connection_file(world, &azure_entry(&name, &host, None));
}

#[given(
    regex = r#"^une connexion du fichier "([^"]+)" en mode azure vers l'hôte "([^"]+)" avec le mode TLS "([^"]+)"$"#
)]
async fn azure_connection_with_tls(
    world: &mut ConnectionsWorld,
    name: String,
    host: String,
    sslmode: String,
) {
    write_connection_file(world, &azure_entry(&name, &host, Some(&sslmode)));
}

#[given(
    regex = r#"^un espace de travail DBeaver avec une connexion "([^"]+)" au modèle d'authentification "([^"]+)" vers l'hôte "([^"]+)"$"#
)]
async fn workspace_with_auth_model(
    world: &mut ConnectionsWorld,
    name: String,
    model: String,
    host: String,
) {
    let connection = postgres_connection(
        &name,
        json!({ "host": host, "database": "shop", "type": "dev", "auth-model": model }),
    );
    write_dbeaver(
        world,
        "General",
        "",
        &[(
            "pg-1".to_string(),
            connection,
            saved("alice@example.com", "mot-de-passe-dbeaver"),
        )],
    );
}
