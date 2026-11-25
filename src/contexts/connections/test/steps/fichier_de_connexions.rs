use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::domain::errors::ConnectionError;
use scarif_database_mcp::contexts::connections::domain::types::{ExecutionLimits, FindRequest};
use serde_json::{Value, json};

use crate::bdd_support::{database_target, offline_target};
use crate::connections_steps::support::{make_executable, write_connection_file, write_dbeaver};
use crate::connections_world::ConnectionsWorld;

const LIMITS: ExecutionLimits = ExecutionLimits {
    max_rows: 10,
    timeout_ms: 10_000,
};

fn docstring(step: &Step) -> &str {
    step.docstring
        .as_deref()
        .expect("a docstring")
        .trim_start_matches('\n')
}

fn key_of(line: &str) -> &str {
    line.split('=').next().unwrap_or_default().trim()
}

#[given(regex = r"^le fichier de connexions suivant :$")]
async fn connection_file(world: &mut ConnectionsWorld, step: &Step) {
    write_connection_file(world, docstring(step));
}

#[when(regex = r"^le fichier de connexions devient :$")]
async fn connection_file_becomes(world: &mut ConnectionsWorld, step: &Step) {
    write_connection_file(world, docstring(step));
}

#[given(
    regex = r#"^le fichier de connexions qui déclare l'entrée suivante sans son champ "([^"]+)" :$"#
)]
async fn entry_without_field(world: &mut ConnectionsWorld, field: String, step: &Step) {
    let entry = docstring(step);
    assert!(
        entry.lines().any(|line| key_of(line) == field),
        "the entry must declare {field} before the scenario drops it"
    );
    let kept: Vec<&str> = entry.lines().filter(|line| key_of(line) != field).collect();
    write_connection_file(world, &kept.join("\n"));
}

#[given(
    regex = r"^le fichier de connexions qui déclare l'entrée suivante, où la ligne « (.+) » remplace celle de même clé :$"
)]
async fn entry_with_line(world: &mut ConnectionsWorld, line: String, step: &Step) {
    let key = key_of(&line).to_string();
    let mut kept: Vec<String> = docstring(step)
        .lines()
        .filter(|existing| key_of(existing) != key)
        .map(str::to_string)
        .collect();
    kept.push(line);
    write_connection_file(world, &kept.join("\n"));
}

#[given(regex = r"^le fichier de connexions qui contient seulement la ligne « (.+) »$")]
async fn file_with_single_line(world: &mut ConnectionsWorld, line: String) {
    write_connection_file(world, &format!("{line}\n"));
}

#[given(regex = r"^aucun fichier de connexions$")]
async fn no_connection_file(world: &mut ConnectionsWorld) {
    std::fs::remove_file(world.connections_file()).expect("remove connection file");
}

#[given(regex = r"^un fichier de connexions illisible$")]
async fn unreadable_connection_file(world: &mut ConnectionsWorld) {
    let file = world.connections_file();
    std::fs::remove_file(&file).expect("remove connection file");
    std::fs::create_dir(&file).expect("a directory where the file should be");
}

#[given(
    regex = r#"^un espace de travail DBeaver avec les connexions "([^"]+)" et "([^"]+)" dont les mots de passe sont enregistrés$"#
)]
async fn workspace_with_two_connections(
    world: &mut ConnectionsWorld,
    first: String,
    second: String,
) {
    let target = offline_target();
    let connections: Vec<(String, Value, Value)> = [first, second]
        .into_iter()
        .map(|name| {
            (
                format!("postgres-jdbc-{name}"),
                json!({
                    "provider": "postgresql",
                    "driver": "postgres-jdbc",
                    "name": name,
                    "save-password": true,
                    "configuration": {
                        "host": target.host,
                        "port": target.port,
                        "database": target.database,
                        "type": "dev",
                        "auth-model": "native",
                        "provider-properties": { "sslmode": "disable" }
                    }
                }),
                json!({ "#connection": { "user": target.user, "password": target.password } }),
            )
        })
        .collect();
    write_dbeaver(world, "General", "", &connections);
}

#[given(regex = r#"^une connexion du fichier "([^"]+)" derrière l'alias SSH "([^"]+)"$"#)]
async fn file_connection_behind_alias(world: &mut ConnectionsWorld, name: String, alias: String) {
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"db.example.com\"\nuser = \"alice\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"s3cret\"]\nssh_alias = \"{alias}\"\n"
        ),
    );
}

#[given(
    regex = r#"^une connexion du fichier "([^"]+)" vers la base de test dont la commande de mot de passe donne d'abord « (.+) » puis « (.+) »$"#
)]
async fn file_connection_with_scripted_command(
    world: &mut ConnectionsWorld,
    name: String,
    first: String,
    then: String,
) {
    let target = database_target();
    let actual = |password: &str| {
        if password == "le bon mot de passe" {
            target.password.clone()
        } else {
            password.to_string()
        }
    };
    let dir = world.config_dir.path();
    let runs = dir.join("executions");
    let script = format!(
        "echo run >> '{runs}'\nif [ \"$(wc -l < '{runs}')\" -le 1 ]; then printf '%s\\n' '{first}'; else printf '%s\\n' '{then}'; fi\n",
        runs = runs.display(),
        first = actual(&first),
        then = actual(&then),
    );
    let command = dir.join("password-command");
    std::fs::write(&command, format!("#!/bin/sh\n{script}")).expect("password command");
    make_executable(&command);
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"{host}\"\nport = {port}\ndatabase = \"{database}\"\nuser = \"{user}\"\nsslmode = \"disable\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"{command}\"]\n",
            host = target.host,
            port = target.port,
            database = target.database,
            user = target.user,
            command = command.display(),
        ),
    );
}

fn record(world: &mut ConnectionsWorld, outcome: Result<(), ConnectionError>) {
    world.outcome = Some(outcome.clone().map_err(|error| error.to_string()));
    world.failure = outcome.err();
}

#[when(regex = r#"^l'agent exécute une requête de lecture sur la connexion "([^"]+)"$"#)]
async fn agent_runs_read_on(world: &mut ConnectionsWorld, name: String) {
    let context = world.context();
    let outcome = context
        .run_read_only_statement
        .execute(&name, None, "SELECT 1", LIMITS)
        .await
        .map(|_| ());
    record(world, outcome);
}

#[when(
    regex = r#"^l'agent exécute deux requêtes de lecture successives sur la connexion "([^"]+)"$"#
)]
async fn agent_runs_two_reads_on(world: &mut ConnectionsWorld, name: String) {
    let context = world.context();
    let mut outcome = Ok(());
    for _ in 0..2 {
        outcome = outcome.and(
            context
                .run_read_only_statement
                .execute(&name, None, "SELECT 1", LIMITS)
                .await
                .map(|_| ()),
        );
    }
    record(world, outcome);
}

#[when(regex = r#"^l'agent lit le catalogue de la connexion "([^"]+)"$"#)]
async fn agent_reads_catalog_of(world: &mut ConnectionsWorld, name: String) {
    let context = world.context();
    let outcome = context
        .read_catalog
        .execute_cached(&name, None, "SELECT 1", LIMITS)
        .await
        .map(|_| ());
    record(world, outcome);
}

#[when(
    regex = r#"^l'agent lit les documents de la collection "([^"]+)" sur la connexion "([^"]+)"$"#
)]
async fn agent_finds_on(world: &mut ConnectionsWorld, collection: String, name: String) {
    let context = world.context();
    let request = FindRequest {
        database: Some("shop".to_string()),
        collection,
        filter: json!({}),
        projection: None,
        sort: None,
        skip: None,
        limit: None,
        hint: None,
        collation: None,
    };
    let outcome = context
        .run_find
        .execute(&name, &request, LIMITS)
        .await
        .map(|_| ());
    record(world, outcome);
}

fn failure_message(world: &ConnectionsWorld) -> &str {
    match &world.outcome {
        Some(Err(message)) => message,
        other => panic!("the query should have failed: {other:?}"),
    }
}

#[then(regex = r#"^la requête échoue avec une erreur qui contient "(.+)"$"#)]
async fn query_fails_with(world: &mut ConnectionsWorld, expected: String) {
    let message = failure_message(world);
    assert!(message.contains(&expected), "{message}");
}

#[then(regex = r#"^l'erreur de la requête ne contient pas "(.+)"$"#)]
async fn query_error_does_not_contain(world: &mut ConnectionsWorld, forbidden: String) {
    let message = failure_message(world);
    assert!(!message.contains(&forbidden), "{message}");
}

#[then(
    regex = r"^l'erreur de la requête ne contient ni l'utilisateur ni l'hôte de la base de test$"
)]
async fn query_error_hides_identity(world: &mut ConnectionsWorld) {
    let target = database_target();
    let message = failure_message(world);
    for forbidden in [target.user.as_str(), target.host.as_str()] {
        assert!(!message.contains(forbidden), "{forbidden} in {message}");
    }
}

#[then(regex = r#"^l'erreur propose les connexions disponibles "([^"]+)"$"#)]
async fn error_lists_available(world: &mut ConnectionsWorld, expected: String) {
    let expected: Vec<String> = expected.split(", ").map(str::to_string).collect();
    match &world.failure {
        Some(ConnectionError::Unknown { available, .. }) => assert_eq!(available, &expected),
        other => panic!("expected an unknown connection error, got {other:?}"),
    }
}

#[then(regex = r"^la commande de mot de passe n'a pas été exécutée$")]
async fn command_was_not_run(world: &mut ConnectionsWorld) {
    assert!(!world.config_dir.path().join("marqueur").exists());
}

#[then(regex = r"^la commande de mot de passe a été exécutée (\d+) fois$")]
async fn command_ran(world: &mut ConnectionsWorld, expected: usize) {
    let runs = std::fs::read_to_string(world.config_dir.path().join("executions"))
        .unwrap_or_default()
        .lines()
        .count();
    assert_eq!(runs, expected);
}
