use cucumber::{given, then, when};
use scarif_database_mcp::contexts::connections::infrastructure::data_sources::password_command::{
    PasswordCommandRunner, ProcessPasswordCommand,
};
use serde_json::json;

use crate::bdd_support::database_target;
use crate::connections_steps::support::{
    LIMITS, failure_message, make_executable, record, write_connection_file,
};
use crate::connections_world::ConnectionsWorld;

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

#[then(regex = r"^la commande de mot de passe a été exécutée (\d+) fois$")]
async fn command_ran(world: &mut ConnectionsWorld, expected: usize) {
    let runs = std::fs::read_to_string(world.config_dir.path().join("executions"))
        .unwrap_or_default()
        .lines()
        .count();
    assert_eq!(runs, expected);
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
