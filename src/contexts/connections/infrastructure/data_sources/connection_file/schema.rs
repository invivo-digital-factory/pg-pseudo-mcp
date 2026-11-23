use std::collections::HashMap;

use toml::{Table, Value};

use crate::contexts::connections::domain::types::{
    AuthMode, ConnectionDefinition, ConnectionType, SslMode,
};

const DEFAULT_PORT: u16 = 5432;
const DEFAULT_DATABASE: &str = "postgres";
const KNOWN_KEYS: [&str; 10] = [
    "name",
    "host",
    "port",
    "database",
    "user",
    "sslmode",
    "type",
    "auth",
    "ssh_alias",
    "password_command",
];
const SECRET_FRAGMENTS: [&str; 5] = ["password", "passwd", "token", "passphrase", "secret"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawEntry {
    pub name: String,
    pub definition: Result<ConnectionDefinition, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbeaverAccess {
    Read,
    Off,
    Blocked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFile {
    pub dbeaver: DbeaverAccess,
    pub entries: Vec<RawEntry>,
    pub notes: Vec<String>,
}

pub fn parse_file(contents: &str) -> Result<ParsedFile, SyntaxError> {
    let table: Table = toml::from_str(contents).map_err(|error| {
        let offset = error.span().map_or(0, |span| span.start);
        position_of(contents, offset)
    })?;
    let mut notes = Vec::new();
    let mut read_dbeaver = true;
    let mut file_level_problem = false;
    let mut tables: Vec<&Table> = Vec::new();
    for (key, value) in &table {
        match (key.as_str(), value) {
            ("read_dbeaver", Value::Boolean(flag)) => read_dbeaver = *flag,
            ("read_dbeaver", _) => {
                file_level_problem = true;
                notes.push(
                    "`read_dbeaver` must be true or false; DBeaver connections are unavailable until it is fixed"
                        .to_string(),
                );
            }
            ("connection", Value::Array(items)) => {
                for item in items {
                    match item {
                        Value::Table(entry) => tables.push(entry),
                        _ => notes.push("each `connection` must be a table".to_string()),
                    }
                }
            }
            ("connection", _) => notes.push(
                "`connection` must be an array of tables, written [[connection]]".to_string(),
            ),
            (other, _) => {
                file_level_problem = true;
                notes.push(top_level_note(other));
            }
        }
    }
    let mut entries: Vec<RawEntry> = tables
        .iter()
        .enumerate()
        .map(|(index, entry)| raw_entry(entry, index + 1))
        .collect();
    flag_duplicates(&mut entries);
    let dbeaver = match (read_dbeaver, file_level_problem) {
        (false, _) => DbeaverAccess::Off,
        (true, true) => DbeaverAccess::Blocked,
        (true, false) => DbeaverAccess::Read,
    };
    Ok(ParsedFile {
        dbeaver,
        entries,
        notes,
    })
}

pub fn position_of(contents: &str, offset: usize) -> SyntaxError {
    let end = offset.min(contents.len());
    let before = contents.get(..end).unwrap_or_default();
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(1, |last| last.chars().count() + 1);
    SyntaxError { line, column }
}

fn top_level_note(key: &str) -> String {
    if is_secret_key(key) {
        format!("the connection file must not hold secrets; remove the top-level key `{key}`")
    } else {
        format!("unknown top-level key `{key}` in the connection file")
    }
}

fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered != "password_command"
        && SECRET_FRAGMENTS
            .iter()
            .any(|fragment| lowered.contains(fragment))
}

fn raw_entry(table: &Table, index: usize) -> RawEntry {
    let name = table
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string);
    let definition = definition_of(table);
    RawEntry {
        name: name.unwrap_or_else(|| format!("connection #{index}")),
        definition,
    }
}

fn flag_duplicates(entries: &mut [RawEntry]) {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for entry in entries.iter() {
        *counts.entry(entry.name.clone()).or_default() += 1;
    }
    for entry in entries.iter_mut() {
        if counts.get(&entry.name).copied().unwrap_or(0) > 1 {
            entry.definition = Err(format!(
                "the name `{}` is defined more than once in the connection file",
                entry.name
            ));
        }
    }
}

fn definition_of(table: &Table) -> Result<ConnectionDefinition, String> {
    if let Some(key) = table.keys().find(|key| is_secret_key(key)) {
        return Err(format!(
            "the connection file must not hold secrets; remove `{key}` (passwords come from pgpass, the Azure CLI or password_command)"
        ));
    }
    if let Some(key) = table.keys().find(|key| !KNOWN_KEYS.contains(&key.as_str())) {
        return Err(format!("unknown key `{key}` in the connection file"));
    }
    let name = required_text(table, "name")?;
    let host = required_text(table, "host")?;
    let user = required_text(table, "user")?;
    let database =
        optional_text(table, "database")?.unwrap_or_else(|| DEFAULT_DATABASE.to_string());
    let port = match table.get("port") {
        None => DEFAULT_PORT,
        Some(Value::Integer(port)) => u16::try_from(*port)
            .ok()
            .filter(|port| *port != 0)
            .ok_or("`port` must be between 1 and 65535")?,
        Some(_) => return Err("`port` must be a whole number".to_string()),
    };
    let sslmode = match optional_text(table, "sslmode")? {
        None => SslMode::Prefer,
        Some(text) => SslMode::from_name(&text).ok_or(
            "`sslmode` must be one of disable, allow, prefer, require, verify-ca, verify-full",
        )?,
    };
    let connection_type = ConnectionType::from_name(&required_text(table, "type")?)
        .ok_or("`type` must be one of dev, test, prod")?;
    let auth = auth_of(table)?;
    let ssh_alias = optional_text(table, "ssh_alias")?;
    if let Some(alias) = &ssh_alias
        && (alias.starts_with('-') || alias.contains(char::is_whitespace))
    {
        return Err("`ssh_alias` must be a host alias of ~/.ssh/config".to_string());
    }
    Ok(ConnectionDefinition {
        name,
        host,
        port,
        database,
        user,
        sslmode,
        connection_type,
        auth,
        ssh_alias,
    })
}

fn auth_of(table: &Table) -> Result<AuthMode, String> {
    let command = match table.get("password_command") {
        None => None,
        Some(Value::Array(items)) => {
            let argv: Option<Vec<String>> = items
                .iter()
                .map(|item| {
                    item.as_str()
                        .filter(|text| !text.is_empty())
                        .map(str::to_string)
                })
                .collect();
            match argv {
                Some(argv) if !argv.is_empty() => Some(argv),
                _ => {
                    return Err(
                        "`password_command` must be a non-empty list of non-empty strings"
                            .to_string(),
                    );
                }
            }
        }
        Some(_) => {
            return Err(
                "`password_command` must be a list: the program, then its arguments".to_string(),
            );
        }
    };
    match (required_text(table, "auth")?.as_str(), command) {
        ("pgpass", None) => Ok(AuthMode::Pgpass),
        ("azure", None) => Ok(AuthMode::Azure),
        ("command", Some(argv)) => Ok(AuthMode::Command(argv)),
        ("command", None) => {
            Err("`password_command` is required when auth is \"command\"".to_string())
        }
        ("pgpass" | "azure", Some(_)) => {
            Err("`password_command` is only allowed when auth is \"command\"".to_string())
        }
        _ => Err("`auth` must be one of pgpass, azure, command".to_string()),
    }
}

fn required_text(table: &Table, key: &str) -> Result<String, String> {
    optional_text(table, key)?.ok_or_else(|| format!("`{key}` is required"))
}

fn optional_text(table: &Table, key: &str) -> Result<Option<String>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(Some(text.clone())),
        Some(_) => Err(format!("`{key}` must be a non-empty string")),
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    const VALID: &str = r#"
[[connection]]
name = "app-prod"
host = "db.example"
user = "alice"
type = "prod"
auth = "pgpass"
"#;

    fn only_definition(contents: &str) -> Result<ConnectionDefinition, String> {
        let parsed = parse_file(contents).expect("valid toml");
        assert_eq!(parsed.entries.len(), 1, "{contents}");
        parsed.entries.into_iter().next().unwrap().definition
    }

    fn entry_with(extra: &str) -> String {
        format!("{VALID}{extra}\n")
    }

    #[test]
    fn une_connexion_minimale_recoit_les_valeurs_par_defaut() {
        let definition = only_definition(VALID).unwrap();
        assert_eq!(definition.port, 5432);
        assert_eq!(definition.database, "postgres");
        assert_eq!(definition.sslmode, SslMode::Prefer);
        assert_eq!(definition.auth, AuthMode::Pgpass);
        assert_eq!(definition.ssh_alias, None);
        assert_eq!(definition.connection_type, ConnectionType::Prod);
    }

    #[test]
    fn une_connexion_complete_est_lue_champ_par_champ() {
        let definition = only_definition(
            r#"
[[connection]]
name = "billing"
host = "db.example"
port = 6432
database = "billing"
user = "alice"
sslmode = "verify-full"
type = "test"
auth = "command"
password_command = ["op", "read", "op://vault/db"]
ssh_alias = "bastion"
"#,
        )
        .unwrap();
        assert_eq!(definition.port, 6432);
        assert_eq!(definition.database, "billing");
        assert_eq!(definition.sslmode, SslMode::VerifyFull);
        assert_eq!(
            definition.auth,
            AuthMode::Command(vec!["op".into(), "read".into(), "op://vault/db".into()])
        );
        assert_eq!(definition.ssh_alias.as_deref(), Some("bastion"));
    }

    #[test]
    fn toute_cle_qui_ressemble_a_un_secret_rend_la_connexion_indisponible_avec_la_raison_des_secrets()
     {
        for key in [
            "password",
            "passwd",
            "pgpassword",
            "token",
            "access_token",
            "passphrase",
            "secret",
            "db_password",
            "Password",
        ] {
            let reason = only_definition(&entry_with(&format!("{key} = \"ab-1234\""))).unwrap_err();
            assert!(reason.contains("must not hold secrets"), "{key}: {reason}");
            assert!(reason.contains(key), "{key}: {reason}");
            assert!(!reason.contains("ab-1234"), "{key}: {reason}");
        }
    }

    #[test]
    fn une_cle_inconnue_est_nommee_dans_la_raison() {
        let reason = only_definition(&entry_with("colour = \"blue\"")).unwrap_err();
        assert!(reason.contains("unknown key `colour`"), "{reason}");
    }

    #[test]
    fn les_valeurs_invalides_nomment_le_champ_fautif() {
        for (extra, field) in [
            ("port = 0", "port"),
            ("port = 70000", "port"),
            ("port = \"5432\"", "port"),
            ("sslmode = \"sometimes\"", "sslmode"),
            ("database = \"\"", "database"),
            ("ssh_alias = \"-oProxyCommand=x\"", "ssh_alias"),
            ("ssh_alias = \"two words\"", "ssh_alias"),
        ] {
            let reason = only_definition(&entry_with(extra)).unwrap_err();
            assert!(reason.contains(&format!("`{field}`")), "{extra}: {reason}");
        }
        let reason = only_definition(&VALID.replace("prod", "staging")).unwrap_err();
        assert!(reason.contains("`type`"), "{reason}");
        let reason = only_definition(&VALID.replace("pgpass", "kerberos")).unwrap_err();
        assert!(reason.contains("`auth`"), "{reason}");
    }

    #[test]
    fn les_champs_obligatoires_manquants_sont_nommes() {
        for field in ["host", "user", "type", "auth"] {
            let contents: String = VALID
                .lines()
                .filter(|line| !line.starts_with(&format!("{field} =")))
                .collect::<Vec<_>>()
                .join("\n");
            let reason = only_definition(&contents).unwrap_err();
            assert!(
                reason.contains(&format!("`{field}` is required")),
                "{reason}"
            );
        }
    }

    #[test]
    fn la_commande_de_mot_de_passe_est_exigee_par_auth_command_et_interdite_sinon() {
        let reason = only_definition(&VALID.replace("pgpass", "command")).unwrap_err();
        assert!(
            reason.contains("`password_command` is required"),
            "{reason}"
        );
        let reason = only_definition(&entry_with("password_command = [\"op\"]")).unwrap_err();
        assert!(reason.contains("only allowed"), "{reason}");
        for bad in ["[]", "[\"\"]", "[1]", "\"op read\""] {
            let contents =
                VALID.replace("pgpass", "command") + &format!("password_command = {bad}\n");
            let reason = only_definition(&contents).unwrap_err();
            assert!(reason.contains("`password_command`"), "{bad}: {reason}");
        }
    }

    #[test]
    fn deux_entrees_du_meme_nom_sont_toutes_deux_marquees_comme_doublons() {
        let contents = format!("{VALID}{VALID}");
        let parsed = parse_file(&contents).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        for entry in &parsed.entries {
            let reason = entry.definition.as_ref().unwrap_err();
            assert!(reason.contains("`app-prod`"), "{reason}");
            assert!(reason.contains("more than once"), "{reason}");
        }
    }

    #[test]
    fn une_entree_sans_nom_valide_recoit_un_nom_de_position() {
        let parsed = parse_file("[[connection]]\nhost = \"h\"\n").unwrap();
        assert_eq!(parsed.entries[0].name, "connection #1");
        assert!(parsed.entries[0].definition.is_err());
    }

    #[test]
    fn read_dbeaver_false_lisible_coupe_dbeaver_et_la_valeur_par_defaut_le_laisse_lu() {
        assert_eq!(parse_file(VALID).unwrap().dbeaver, DbeaverAccess::Read);
        let parsed = parse_file(&format!("read_dbeaver = false\n{VALID}")).unwrap();
        assert_eq!(parsed.dbeaver, DbeaverAccess::Off);
        let parsed = parse_file(&format!("read_dbeaver = false\ncolour = 1\n{VALID}")).unwrap();
        assert_eq!(parsed.dbeaver, DbeaverAccess::Off);
    }

    #[test]
    fn une_valeur_invalide_de_read_dbeaver_bloque_dbeaver() {
        let parsed = parse_file(&format!("read_dbeaver = \"no\"\n{VALID}")).unwrap();
        assert_eq!(parsed.dbeaver, DbeaverAccess::Blocked);
        assert!(parsed.notes[0].contains("read_dbeaver"));
    }

    #[test]
    fn une_cle_de_premier_niveau_inconnue_ou_secrete_bloque_dbeaver() {
        for contents in ["colour = 1\n", "password = \"x\"\n", "api_token = \"x\"\n"] {
            let parsed = parse_file(&format!("{contents}{VALID}")).unwrap();
            assert_eq!(parsed.dbeaver, DbeaverAccess::Blocked, "{contents}");
        }
    }

    #[test]
    fn une_cle_de_premier_niveau_inconnue_ou_secrete_est_signalee() {
        let parsed = parse_file("colour = 1\npassword = \"x\"\n").unwrap();
        assert!(parsed.notes.iter().any(|note| note.contains("`colour`")));
        assert!(
            parsed
                .notes
                .iter()
                .any(|note| note.contains("must not hold secrets"))
        );
        assert!(parsed.notes.iter().all(|note| !note.contains("\"x\"")));
    }

    #[test]
    fn une_erreur_de_syntaxe_rend_la_ligne_et_la_colonne_sans_citer_le_contenu() {
        let contents = "[[connection]]\nname = \"a\"\npassword = \"ab\"c\"\n";
        let error = parse_file(contents).unwrap_err();
        assert_eq!(error.line, 3);
        assert!(error.column > 1);
        let shown = format!("{error:?}");
        assert!(!shown.contains("ab"), "{shown}");
    }
}
