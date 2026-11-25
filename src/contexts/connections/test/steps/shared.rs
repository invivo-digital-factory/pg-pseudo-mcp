use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use serde_json::{Map, Value, json};

use crate::bdd_support::{
    CONNECTION, known_hosts_pattern, offline_target, ssh_asset, write_workspace,
};
use crate::connections_steps::support::{
    LIMITS, bastion_key, docstring, entries, failure_message, hint, key_of, list, listed, params,
    postgres_connection, reason, record, saved, tunnel_connection, write_connection_file,
    write_dbeaver, write_pgpass, write_ssh_config,
};
use crate::connections_world::ConnectionsWorld;

#[given(regex = r"^un fichier pgpass qui contient :$")]
async fn pgpass_file(world: &mut ConnectionsWorld, step: &Step) {
    write_pgpass(
        world,
        step.docstring.as_deref().expect("a docstring"),
        0o600,
    );
}

#[given(
    regex = r#"^un espace de travail DBeaver où la connexion "([^"]+)" a son mot de passe enregistré$"#
)]
async fn workspace_with_saved_password(world: &mut ConnectionsWorld, name: String) {
    assert_eq!(
        name, CONNECTION,
        "the shared workspace names its connection {CONNECTION}"
    );
    write_workspace(world.workspace.path(), &offline_target(), true, None);
}

#[given(
    regex = r#"^un espace de travail DBeaver avec une connexion "([^"]+)" du fournisseur "([^"]+)" au modèle d'authentification "([^"]+)"$"#
)]
async fn workspace_with_provider(
    world: &mut ConnectionsWorld,
    name: String,
    provider: String,
    model: String,
) {
    let mut connection = postgres_connection(
        &name,
        json!({ "host": "db.example.com", "database": "shop", "type": "dev", "auth-model": model }),
    );
    connection["provider"] = json!(provider);
    write_dbeaver(
        world,
        "General",
        "",
        &[("conn-1".to_string(), connection, saved("alice", "s3cret"))],
    );
}

#[given(
    regex = r#"^un espace de travail DBeaver dont la connexion "([^"]+)" passe par un tunnel SSH en "([^"]+)" avec la clé "([^"]*)", l'utilisateur SSH "([^"]*)" et le mot de passe SSH "([^"]*)"$"#
)]
async fn workspace_with_tunnel(
    world: &mut ConnectionsWorld,
    name: String,
    auth: String,
    key: String,
    user: String,
    password: String,
) {
    let home = std::env::var("HOME").unwrap_or_default();
    let key = key
        .replace(
            "{clé de test}",
            &ssh_asset("tunnel_test_ed25519").to_string_lossy(),
        )
        .replace("{dossier personnel}", &home);
    let mut properties = json!({ "authType": auth, "host": "bastion.example.com", "port": 22 });
    if !key.is_empty() {
        properties["keyPath"] = json!(key);
    }
    let mut ssh = Map::new();
    if !user.is_empty() {
        ssh.insert("user".to_string(), json!(user));
    }
    if !password.is_empty() {
        ssh.insert("password".to_string(), json!(password));
    }
    tunnel_connection(
        world,
        &name,
        json!({ "ssh_tunnel": { "type": "TUNNEL", "enabled": true, "properties": properties } }),
        Value::Object(ssh),
    );
}

#[given(regex = r#"^un espace de travail DBeaver avec deux connexions nommées "([^"]+)"$"#)]
async fn workspace_with_duplicates(world: &mut ConnectionsWorld, name: String) {
    let connections: Vec<(String, Value, Value)> = ["pg-1", "pg-2"]
        .into_iter()
        .map(|id| {
            (
                id.to_string(),
                postgres_connection(
                    &name,
                    json!({ "host": "db.example.com", "port": "5432", "database": "app",
                            "type": "dev", "auth-model": "native" }),
                ),
                saved("alice", "dbeaver-saved-canary-7741"),
            )
        })
        .collect();
    write_dbeaver(world, "General", "", &connections);
}

#[when(regex = r"^l'agent liste les connexions$")]
async fn agent_lists_connections(world: &mut ConnectionsWorld) {
    list(world).await;
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme disponible$"#)]
async fn connection_is_available(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], "available", "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme indisponible$"#)]
async fn connection_is_unavailable(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], "unavailable", "{entry}");
}

#[then(regex = r#"^la connexion "([^"]+)" est listée comme non prise en charge$"#)]
async fn connection_is_unsupported(world: &mut ConnectionsWorld, name: String) {
    let entry = listed(world, &name);
    assert_eq!(entry["status"], "unsupported", "{entry}");
}

#[then(
    regex = r#"^la connexion "([^"]+)" est listée avec (la source|le moteur|la base|le type) "([^"]+)"$"#
)]
async fn connection_has_field(
    world: &mut ConnectionsWorld,
    name: String,
    field: String,
    value: String,
) {
    let key = match field.as_str() {
        "la source" => "source",
        "le moteur" => "engine",
        "la base" => "database",
        _ => "type",
    };
    let entry = listed(world, &name);
    assert_eq!(entry[key], value, "{entry}");
}

#[then(regex = r#"^la liste ne contient pas la connexion "([^"]+)"$"#)]
async fn connection_not_listed(world: &mut ConnectionsWorld, name: String) {
    assert!(entries(world, &name).is_empty(), "{name} is listed");
}

#[then(regex = r#"^la raison de la connexion "([^"]+)" contient "(.+)"$"#)]
async fn reason_contains(world: &mut ConnectionsWorld, name: String, expected: String) {
    let reason = reason(world, &name);
    assert!(reason.contains(&expected), "{reason}");
}

#[then(
    regex = r#"^la liste ne montre ni l'hôte "([^"]+)", ni l'utilisateur "([^"]+)", ni le secret "([^"]+)"$"#
)]
async fn listing_hides(world: &mut ConnectionsWorld, host: String, user: String, secret: String) {
    let raw = world.listing.as_ref().expect("listed").to_string();
    for forbidden in [host, user, secret] {
        assert!(!raw.contains(&forbidden), "{forbidden} leaked in {raw}");
    }
}

#[then(
    regex = r#"^la vérification de clé d'hôte de la connexion "([^"]+)" est listée comme "([^"]+)"$"#
)]
async fn host_key_check_flag(world: &mut ConnectionsWorld, name: String, flag: String) {
    let entry = listed(world, &name);
    match flag.as_str() {
        "non signalé" => assert!(entry.get("ssh_host_key_check").is_none(), "{entry}"),
        value => assert_eq!(entry["ssh_host_key_check"], value, "{entry}"),
    }
}

#[then(regex = r"^l'indice cite la ligne (\d+)$")]
async fn hint_names_line(world: &mut ConnectionsWorld, line: usize) {
    let hint = hint(world);
    assert!(hint.contains(&format!("line {line},")), "{hint}");
}

#[then(regex = r#"^l'indice contient "(.+)"$"#)]
async fn hint_contains(world: &mut ConnectionsWorld, expected: String) {
    let hint = hint(world);
    assert!(hint.contains(&expected), "{hint}");
}

#[then(regex = r#"^l'indice ne contient pas "(.+)"$"#)]
async fn hint_does_not_contain(world: &mut ConnectionsWorld, forbidden: String) {
    let hint = hint(world);
    assert!(!hint.contains(&forbidden), "{hint}");
}

#[then(regex = r"^l'indice propose de créer le fichier ou de lancer l'import$")]
async fn hint_proposes_file_or_import(world: &mut ConnectionsWorld) {
    let hint = hint(world);
    assert!(hint.contains("connections.toml"), "{hint}");
    assert!(
        hint.contains("run `scarif-database-mcp connections import-dbeaver`"),
        "{hint}"
    );
}

#[given(regex = r"^le fichier de connexions suivant :$")]
async fn connection_file(world: &mut ConnectionsWorld, step: &Step) {
    write_connection_file(world, docstring(step));
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

#[given(regex = r#"^une connexion du fichier "([^"]+)" derrière l'alias SSH "([^"]+)"$"#)]
async fn file_connection_behind_alias(world: &mut ConnectionsWorld, name: String, alias: String) {
    write_connection_file(
        world,
        &format!(
            "[[connection]]\nname = \"{name}\"\nhost = \"db.example.com\"\nuser = \"alice\"\ntype = \"dev\"\nauth = \"command\"\npassword_command = [\"/bin/echo\", \"s3cret\"]\nssh_alias = \"{alias}\"\n"
        ),
    );
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

#[then(regex = r"^la commande de mot de passe n'a pas été exécutée$")]
async fn command_was_not_run(world: &mut ConnectionsWorld) {
    assert!(!world.config_dir.path().join("marqueur").exists());
}

#[then(regex = r#"^la connexion "([^"]+)" vise le port (\d+) avec le mode TLS "([^"]+)"$"#)]
async fn targets_port_and_tls(world: &mut ConnectionsWorld, name: String, port: u16, tls: String) {
    let params = params(world, &name);
    assert_eq!(params.port, port);
    assert_eq!(params.sslmode.as_str(), tls);
}

#[then(regex = r#"^le mot de passe retenu pour la connexion "([^"]+)" est « (.+) »$"#)]
async fn retained_password(world: &mut ConnectionsWorld, name: String, password: String) {
    assert_eq!(params(world, &name).password.expose(), password);
}

#[given(regex = r"^un fichier de configuration SSH qui contient :$")]
async fn ssh_config_contents(world: &mut ConnectionsWorld, step: &Step) {
    write_ssh_config(world, step.docstring.as_deref().expect("a docstring"));
}

#[given(regex = r"^un fichier known_hosts qui connaît la clé du bastion$")]
async fn known_hosts_with_bastion_key(world: &mut ConnectionsWorld) {
    world.write_known_hosts(&format!("{} {}\n", known_hosts_pattern(), bastion_key()));
}

#[then(regex = r"^la session s'ouvre et répond$")]
async fn session_answers(world: &mut ConnectionsWorld) {
    assert_eq!(world.outcome, Some(Ok(())));
}

fn refusal(world: &ConnectionsWorld) -> &str {
    match world.outcome.as_ref().expect("a statement was run") {
        Ok(()) => panic!("the connection was accepted"),
        Err(message) => message,
    }
}

#[then(regex = r"^la connexion est refusée$")]
async fn connection_is_refused(world: &mut ConnectionsWorld) {
    refusal(world);
}

#[then(regex = r#"^le message de refus contient "(.+)"$"#)]
async fn refusal_contains(world: &mut ConnectionsWorld, expected: String) {
    let message = refusal(world);
    assert!(message.contains(&expected), "{message}");
}

#[then(regex = r#"^le message de refus contient l'empreinte "([^"]+)" de la clé du bastion$"#)]
async fn refusal_carries_fingerprint(world: &mut ConnectionsWorld, prefix: String) {
    let message = refusal(world);
    assert!(
        message.contains(&format!("ssh-ed25519 {prefix}")),
        "{message}"
    );
}
