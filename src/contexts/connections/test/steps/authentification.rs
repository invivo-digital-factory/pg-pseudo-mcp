use std::os::unix::fs::PermissionsExt;

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::password_command::{
    PasswordCommandRunner, ProcessPasswordCommand,
};
use serde_json::json;

use crate::connections_steps::support::{make_executable, write_connection_file};
use crate::connections_world::ConnectionsWorld;

fn write_pgpass(world: &ConnectionsWorld, contents: &str, mode: u32) {
    let path = world.pgpass_file();
    std::fs::write(&path, contents).expect("pgpass file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).expect("pgpass mode");
}

#[given(regex = r"^un fichier pgpass qui contient :$")]
async fn pgpass_file(world: &mut ConnectionsWorld, step: &Step) {
    write_pgpass(
        world,
        step.docstring.as_deref().expect("a docstring"),
        0o600,
    );
}

#[given(regex = r"^un fichier pgpass aux droits (\d+) qui contient :$")]
async fn pgpass_file_with_mode(world: &mut ConnectionsWorld, mode: String, step: &Step) {
    let mode = u32::from_str_radix(&mode, 8).expect("octal mode");
    write_pgpass(world, step.docstring.as_deref().expect("a docstring"), mode);
}

#[given(regex = r"^un fichier pgpass remplacé par un dossier qui contient :$")]
async fn pgpass_directory(world: &mut ConnectionsWorld, step: &Step) {
    let dir = world.pgpass_file();
    std::fs::create_dir(&dir).expect("pgpass directory");
    std::fs::write(
        dir.join("pgpass"),
        step.docstring.as_deref().expect("a docstring"),
    )
    .expect("file inside the pgpass directory");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("dir mode");
}

#[given(
    regex = r#"^le fichier de connexions qui déclare la connexion "([^"]+)" vers la base "([^"]+)" en mode pgpass$"#
)]
async fn file_pgpass_connection(world: &mut ConnectionsWorld, name: String, database: String) {
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"db.example.com\"\ndatabase = \"{database}\"\nuser = \"alice\"\ntype = \"dev\"\nauth = \"pgpass\"\n"
        ),
    );
}

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

fn command_line(text: &str) -> Vec<String> {
    text.trim()
        .strip_prefix("[\"")
        .and_then(|rest| rest.strip_suffix("\"]"))
        .expect("a list of quoted words")
        .split("\", \"")
        .map(str::to_string)
        .collect()
}

#[when(regex = r"^le serveur lance la commande de mot de passe (\[.*\])$")]
async fn run_password_command(world: &mut ConnectionsWorld, command: String) {
    let outcome = ProcessPasswordCommand::new().run(&command_line(&command));
    world.probe = Some(match outcome {
        Ok(secret) => json!({ "password": secret.expose() }),
        Err(error) => json!({ "error": error.to_string() }),
    });
}

#[when(
    regex = r"^le serveur lance la commande de mot de passe (\[.*\]) avec un délai réduit à (\d+) seconde$"
)]
async fn run_password_command_with_deadline(
    world: &mut ConnectionsWorld,
    command: String,
    seconds: u64,
) {
    let started = std::time::Instant::now();
    let outcome = ProcessPasswordCommand::with_deadline(std::time::Duration::from_secs(seconds))
        .run(&command_line(&command));
    assert!(
        started.elapsed() < std::time::Duration::from_secs(seconds + 3),
        "the command was not stopped at its deadline"
    );
    world.probe = Some(match outcome {
        Ok(secret) => json!({ "password": secret.expose() }),
        Err(error) => json!({ "error": error.to_string() }),
    });
}

#[then(regex = r"^la commande donne le mot de passe « (.+) »$")]
async fn command_gives(world: &mut ConnectionsWorld, expected: String) {
    let probe = world.probe.as_ref().expect("a command was run");
    assert_eq!(probe["password"], expected.replace('⏎', "\n"), "{probe}");
}

#[then(regex = r"^la commande échoue avec le motif « (.+) »$")]
async fn command_fails_with(world: &mut ConnectionsWorld, expected: String) {
    let probe = world.probe.as_ref().expect("a command was run");
    assert_eq!(probe["error"], expected, "{probe}");
}
