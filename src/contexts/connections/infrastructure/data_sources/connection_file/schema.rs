use std::collections::HashMap;

use toml::{Table, Value};

use crate::contexts::connections::domain::constants::{ENGINE_MONGODB, ENGINE_POSTGRESQL};
use crate::contexts::connections::domain::types::{
    AuthMode, ConnectionDefinition, ConnectionType, EngineSettings, MongoReadPreference,
    MongoSettings, MongoTransport, ReadPreferenceMode, SslMode,
};

const DEFAULT_POSTGRES_PORT: u16 = 5432;
const DEFAULT_MONGODB_PORT: u16 = 27017;
const DEFAULT_DATABASE: &str = "postgres";
const MONGODB_ONLY_KEYS: [&str; 5] = [
    "srv",
    "tls",
    "replica_set",
    "read_preference",
    "read_preference_tags",
];
const KNOWN_KEYS: [&str; 16] = [
    "name",
    "engine",
    "srv",
    "tls",
    "replica_set",
    "read_preference",
    "read_preference_tags",
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
    pub engine: &'static str,
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
                        _ => {
                            file_level_problem = true;
                            notes.push(
                                "each `connection` must be a table; DBeaver connections are unavailable until it is fixed"
                                    .to_string(),
                            );
                        }
                    }
                }
            }
            ("connection", _) => {
                file_level_problem = true;
                notes.push(
                    "`connection` must be an array of tables, written [[connection]]; DBeaver connections are unavailable until it is fixed"
                        .to_string(),
                );
            }
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
    let engine = match table.get("engine").and_then(Value::as_str) {
        Some(ENGINE_MONGODB) => ENGINE_MONGODB,
        _ => ENGINE_POSTGRESQL,
    };
    let definition = definition_of(table);
    RawEntry {
        name: name.unwrap_or_else(|| format!("connection #{index}")),
        engine,
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
    let engine = optional_text(table, "engine")?.unwrap_or_else(|| ENGINE_POSTGRESQL.to_string());
    let mongodb = match engine.as_str() {
        ENGINE_POSTGRESQL => false,
        ENGINE_MONGODB => true,
        _ => return Err("`engine` must be one of postgresql, mongodb".to_string()),
    };
    let name = required_text(table, "name")?;
    let host = required_text(table, "host")?;
    let user = required_text(table, "user")?;
    let port = port_of(table)?;
    let settings = if mongodb {
        EngineSettings::Mongodb(mongo_settings_of(table, host, port)?)
    } else {
        postgres_settings_of(table, host, port.unwrap_or(DEFAULT_POSTGRES_PORT))?
    };
    let connection_type = ConnectionType::from_name(&required_text(table, "type")?)
        .ok_or("`type` must be one of dev, test, prod")?;
    let auth = auth_of(table, mongodb)?;
    let ssh_alias = optional_text(table, "ssh_alias")?;
    if let Some(alias) = &ssh_alias
        && (alias.starts_with('-') || alias.contains(char::is_whitespace))
    {
        return Err("`ssh_alias` must be a host alias of ~/.ssh/config".to_string());
    }
    Ok(ConnectionDefinition {
        name,
        user,
        settings,
        connection_type,
        auth,
        ssh_alias,
    })
}

fn port_of(table: &Table) -> Result<Option<u16>, String> {
    match table.get("port") {
        None => Ok(None),
        Some(Value::Integer(port)) => u16::try_from(*port)
            .ok()
            .filter(|port| *port != 0)
            .map(Some)
            .ok_or_else(|| "`port` must be between 1 and 65535".to_string()),
        Some(_) => Err("`port` must be a whole number".to_string()),
    }
}

fn postgres_settings_of(table: &Table, host: String, port: u16) -> Result<EngineSettings, String> {
    if let Some(key) = MONGODB_ONLY_KEYS
        .iter()
        .find(|key| table.contains_key(**key))
    {
        return Err(format!(
            "`{key}` is only valid when `engine` is \"mongodb\""
        ));
    }
    let database =
        optional_text(table, "database")?.unwrap_or_else(|| DEFAULT_DATABASE.to_string());
    let sslmode = match optional_text(table, "sslmode")? {
        None => SslMode::Prefer,
        Some(text) => SslMode::from_name(&text).ok_or(
            "`sslmode` must be one of disable, allow, prefer, require, verify-ca, verify-full",
        )?,
    };
    Ok(EngineSettings::Postgresql {
        host,
        port,
        database,
        sslmode,
    })
}

fn mongo_settings_of(
    table: &Table,
    host: String,
    port: Option<u16>,
) -> Result<MongoSettings, String> {
    if table.contains_key("sslmode") {
        return Err("`sslmode` is not valid when `engine` is \"mongodb\"; use `tls`".to_string());
    }
    let srv = optional_flag(table, "srv")?.unwrap_or(true);
    let tls = optional_flag(table, "tls")?.unwrap_or(true);
    let transport = match (srv, tls, port) {
        (true, false, _) => {
            return Err(
                "`tls` must be true while `srv` is true; `tls = false` needs `srv = false`"
                    .to_string(),
            );
        }
        (true, true, Some(_)) => {
            return Err("`port` is not valid when `srv` is true; use `srv = false`".to_string());
        }
        (true, true, None) => MongoTransport::Srv { host },
        (false, tls, port) => MongoTransport::Seed {
            host,
            port: port.unwrap_or(DEFAULT_MONGODB_PORT),
            tls,
        },
    };
    Ok(MongoSettings {
        database: optional_text(table, "database")?,
        replica_set: optional_text(table, "replica_set")?,
        transport,
        read_preference: read_preference_of(table)?,
    })
}

fn read_preference_of(table: &Table) -> Result<Option<MongoReadPreference>, String> {
    let mode = match optional_text(table, "read_preference")? {
        None => None,
        Some(name) => Some(ReadPreferenceMode::from_name(&name).ok_or_else(|| {
            format!(
                "`read_preference` must be one of {}",
                ReadPreferenceMode::NAMES.join(", ")
            )
        })?),
    };
    let tags = match table.get("read_preference_tags") {
        None => None,
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .map(|item| item.as_str().and_then(tag_of))
                .collect::<Option<Vec<_>>>()
                .filter(|tags| !tags.is_empty())
                .ok_or(
                    "`read_preference_tags` must be a non-empty list of \"key:value\" strings",
                )?,
        ),
        Some(_) => {
            return Err(
                "`read_preference_tags` must be a non-empty list of \"key:value\" strings"
                    .to_string(),
            );
        }
    };
    match (mode, tags) {
        (None, None) => Ok(None),
        (Some(mode), None) => Ok(Some(MongoReadPreference::of(mode))),
        (None | Some(ReadPreferenceMode::Primary), Some(_)) => {
            Err("`read_preference_tags` needs a `read_preference` other than primary".to_string())
        }
        (Some(mode), Some(tags)) => Ok(Some(MongoReadPreference { mode, tags })),
    }
}

fn tag_of(text: &str) -> Option<(String, String)> {
    let (key, value) = text.split_once(':')?;
    let (key, value) = (key.trim(), value.trim());
    (!key.is_empty() && !value.is_empty()).then(|| (key.to_string(), value.to_string()))
}

fn optional_flag(table: &Table, key: &str) -> Result<Option<bool>, String> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Boolean(flag)) => Ok(Some(*flag)),
        Some(_) => Err(format!("`{key}` must be true or false")),
    }
}

fn auth_of(table: &Table, mongodb: bool) -> Result<AuthMode, String> {
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
    let auth = required_text(table, "auth")?;
    if mongodb && auth != "command" {
        return Err("`auth` must be \"command\" when `engine` is \"mongodb\"".to_string());
    }
    match (auth.as_str(), command) {
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
        assert_eq!(
            definition.settings,
            EngineSettings::Postgresql {
                host: "db.example".to_string(),
                port: 5432,
                database: "postgres".to_string(),
                sslmode: SslMode::Prefer
            }
        );
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
        assert_eq!(
            definition.settings,
            EngineSettings::Postgresql {
                host: "db.example".to_string(),
                port: 6432,
                database: "billing".to_string(),
                sslmode: SslMode::VerifyFull
            }
        );
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

    const MONGO: &str = r#"
[[connection]]
name = "atlas"
engine = "mongodb"
host = "pl-0.atlas.scarif.test"
user = "alice"
type = "dev"
auth = "command"
password_command = ["/bin/echo", "x"]
"#;

    fn mongo_with(extra: &str) -> String {
        format!("{MONGO}{extra}\n")
    }

    #[test]
    fn une_connexion_mongodb_minimale_est_en_srv_avec_tls_sans_port() {
        let definition = only_definition(MONGO).unwrap();
        assert_eq!(
            definition.settings,
            EngineSettings::Mongodb(MongoSettings {
                database: None,
                replica_set: None,
                transport: MongoTransport::Srv {
                    host: "pl-0.atlas.scarif.test".to_string()
                },
                read_preference: None,
            })
        );
    }

    #[test]
    fn une_connexion_mongodb_sans_srv_recoit_le_port_27017_et_tls_par_defaut() {
        let definition = only_definition(&mongo_with("srv = false")).unwrap();
        let EngineSettings::Mongodb(settings) = definition.settings else {
            panic!("expected mongodb settings");
        };
        assert_eq!(
            settings.transport,
            MongoTransport::Seed {
                host: "pl-0.atlas.scarif.test".to_string(),
                port: 27017,
                tls: true
            }
        );
    }

    #[test]
    fn une_connexion_mongodb_complete_est_lue_champ_par_champ() {
        let definition = only_definition(&mongo_with(
            "port = 1024\nsrv = false\ntls = false\nreplica_set = \"rs0\"\ndatabase = \"shop\"\nssh_alias = \"bastion\"",
        ))
        .unwrap();
        assert_eq!(
            definition.settings,
            EngineSettings::Mongodb(MongoSettings {
                database: Some("shop".to_string()),
                replica_set: Some("rs0".to_string()),
                transport: MongoTransport::Seed {
                    host: "pl-0.atlas.scarif.test".to_string(),
                    port: 1024,
                    tls: false
                },
                read_preference: None,
            })
        );
        assert_eq!(definition.ssh_alias.as_deref(), Some("bastion"));
    }

    #[test]
    fn un_port_avec_srv_vrai_explicite_ou_par_defaut_est_refuse_en_nommant_port() {
        for extra in ["port = 1024", "port = 27017\nsrv = true"] {
            let reason = only_definition(&mongo_with(extra)).unwrap_err();
            assert!(reason.contains("`port`"), "{extra}: {reason}");
            assert!(reason.contains("`srv = false`"), "{extra}: {reason}");
        }
    }

    #[test]
    fn le_moteur_postgresql_explicite_equivaut_au_moteur_par_defaut() {
        let explicit =
            only_definition(&VALID.replace("name =", "engine = \"postgresql\"\nname =")).unwrap();
        assert_eq!(explicit, only_definition(VALID).unwrap());
    }

    #[test]
    fn un_moteur_inconnu_ou_mal_type_nomme_engine() {
        for extra in ["engine = \"mysql\"", "engine = 3"] {
            let reason = only_definition(&entry_with(extra)).unwrap_err();
            assert!(reason.contains("`engine`"), "{extra}: {reason}");
        }
    }

    #[test]
    fn une_entree_mongodb_refuse_sslmode_et_les_modes_d_authentification_autres_que_command() {
        let reason = only_definition(&mongo_with("sslmode = \"require\"")).unwrap_err();
        assert!(reason.contains("`sslmode`"), "{reason}");
        for auth in ["pgpass", "azure", "kerberos"] {
            let contents = MONGO
                .replace("auth = \"command\"", &format!("auth = \"{auth}\""))
                .replace("password_command = [\"/bin/echo\", \"x\"]\n", "");
            let reason = only_definition(&contents).unwrap_err();
            assert!(reason.contains("`auth`"), "{auth}: {reason}");
        }
    }

    fn read_preference_of_entry(extra: &str) -> Result<Option<MongoReadPreference>, String> {
        let EngineSettings::Mongodb(settings) = only_definition(&mongo_with(extra))?.settings
        else {
            panic!("expected mongodb settings");
        };
        Ok(settings.read_preference)
    }

    #[test]
    fn sans_read_preference_la_connexion_garde_la_preference_par_defaut() {
        assert_eq!(read_preference_of_entry("").unwrap(), None);
    }

    #[test]
    fn chaque_mode_de_read_preference_est_lu() {
        for name in ReadPreferenceMode::NAMES {
            let read = read_preference_of_entry(&format!("read_preference = \"{name}\""))
                .unwrap()
                .expect(name);
            assert_eq!(read.mode.as_str(), name);
            assert!(read.tags.is_empty(), "{name}");
        }
    }

    #[test]
    fn les_etiquettes_de_read_preference_sont_lues_en_paires_cle_valeur() {
        let read = read_preference_of_entry(
            "read_preference = \"secondary\"\nread_preference_tags = [\"nodeType:ANALYTICS\", \"region: EU \"]",
        )
        .unwrap()
        .unwrap();
        assert_eq!(read.mode, ReadPreferenceMode::Secondary);
        assert_eq!(
            read.tags,
            vec![
                ("nodeType".to_string(), "ANALYTICS".to_string()),
                ("region".to_string(), "EU".to_string())
            ]
        );
    }

    #[test]
    fn une_read_preference_invalide_nomme_la_cle_fautive() {
        for (extra, field) in [
            (
                "read_preference = \"Secondary\"",
                "`read_preference` must be one of",
            ),
            ("read_preference = 1", "`read_preference`"),
            (
                "read_preference = \"secondary\"\nread_preference_tags = []",
                "`read_preference_tags`",
            ),
            (
                "read_preference = \"secondary\"\nread_preference_tags = [\"nodeType\"]",
                "`read_preference_tags`",
            ),
            (
                "read_preference = \"secondary\"\nread_preference_tags = [\":x\"]",
                "`read_preference_tags`",
            ),
            (
                "read_preference = \"secondary\"\nread_preference_tags = \"a:b\"",
                "`read_preference_tags`",
            ),
            (
                "read_preference_tags = [\"a:b\"]",
                "`read_preference_tags` needs a `read_preference` other than primary",
            ),
            (
                "read_preference = \"primary\"\nread_preference_tags = [\"a:b\"]",
                "`read_preference_tags` needs a `read_preference` other than primary",
            ),
        ] {
            let reason = read_preference_of_entry(extra).unwrap_err();
            assert!(reason.contains(field), "{extra}: {reason}");
        }
    }

    #[test]
    fn une_entree_postgresql_refuse_srv_tls_et_replica_set() {
        for extra in [
            "srv = true",
            "tls = true",
            "replica_set = \"rs0\"",
            "read_preference = \"secondary\"",
            "read_preference_tags = [\"a:b\"]",
        ] {
            let key = extra.split(' ').next().unwrap();
            let reason = only_definition(&entry_with(extra)).unwrap_err();
            assert!(reason.contains(&format!("`{key}`")), "{extra}: {reason}");
        }
    }

    #[test]
    fn tls_faux_avec_srv_vrai_est_refuse_et_accepte_sans_srv() {
        let reason = only_definition(&mongo_with("tls = false")).unwrap_err();
        assert!(reason.contains("`tls`"), "{reason}");
        let reason = only_definition(&mongo_with("tls = false\nsrv = true")).unwrap_err();
        assert!(reason.contains("`tls`"), "{reason}");
        assert!(only_definition(&mongo_with("tls = false\nsrv = false")).is_ok());
    }

    #[test]
    fn les_valeurs_mal_typees_des_cles_mongodb_nomment_le_champ_fautif() {
        for (extra, field) in [
            ("srv = \"yes\"", "srv"),
            ("tls = 1", "tls"),
            ("replica_set = \"\"", "replica_set"),
            ("replica_set = 3", "replica_set"),
            ("database = \"\"", "database"),
        ] {
            let reason = only_definition(&mongo_with(extra)).unwrap_err();
            assert!(reason.contains(&format!("`{field}`")), "{extra}: {reason}");
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
    fn une_cle_connection_mal_typee_bloque_dbeaver_et_est_signalee() {
        for contents in [
            "connection = \"x\"\n",
            "connection = 3\n",
            "connection = [1]\n",
            "connection = [\"x\"]\n",
        ] {
            let parsed = parse_file(contents).unwrap();
            assert_eq!(parsed.dbeaver, DbeaverAccess::Blocked, "{contents}");
            assert!(parsed.notes[0].contains("`connection`"), "{contents}");
        }
    }

    #[test]
    fn une_cle_connection_mal_typee_n_emporte_pas_sur_read_dbeaver_false() {
        let parsed = parse_file("read_dbeaver = false\nconnection = \"x\"\n").unwrap();
        assert_eq!(parsed.dbeaver, DbeaverAccess::Off);
    }

    #[test]
    fn une_entree_invalide_d_un_tableau_bien_forme_ne_bloque_pas_dbeaver() {
        let parsed = parse_file("[[connection]]\nname = \"a\"\ncolour = 1\n").unwrap();
        assert_eq!(parsed.dbeaver, DbeaverAccess::Read);
        assert!(parsed.entries[0].definition.is_err());
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
