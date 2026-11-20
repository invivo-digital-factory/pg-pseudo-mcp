use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{AccessTokenFetcher, FetchedToken};
use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::types::Secret;

const COMMAND_DEADLINE: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(20);

pub struct AzureCli {
    command: Option<PathBuf>,
}

#[derive(Deserialize)]
struct CliAnswer {
    #[serde(rename = "accessToken")]
    access_token: String,
    expires_on: Option<u64>,
    #[serde(rename = "expiresOn")]
    expires_on_text: Option<String>,
}

impl AzureCli {
    pub fn new(command: Option<PathBuf>) -> Self {
        Self { command }
    }

    pub fn from_path() -> Self {
        Self::new(find_on_path(std::env::var_os("PATH")))
    }
}

impl AccessTokenFetcher for AzureCli {
    fn fetch(&self) -> Result<FetchedToken, AccessTokenError> {
        let Some(command) = &self.command else {
            return Err(AccessTokenError::CliNotInstalled);
        };
        let output = run(command)?;
        if !output.success {
            return Err(AccessTokenError::SignInRequired);
        }
        let answer: CliAnswer =
            serde_json::from_slice(&output.stdout).map_err(|_| AccessTokenError::Unreadable)?;
        let expires_at = answer
            .expires_on
            .or_else(|| answer.expires_on_text.as_deref().and_then(parse_expiry))
            .ok_or(AccessTokenError::Unreadable)?;
        if answer.access_token.is_empty() {
            return Err(AccessTokenError::Unreadable);
        }
        Ok(FetchedToken {
            token: Secret::new(answer.access_token),
            expires_at,
        })
    }
}

struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
}

fn run(command: &Path) -> Result<CommandOutput, AccessTokenError> {
    let mut child = Command::new(command)
        .args([
            "account",
            "get-access-token",
            "--resource-type",
            "oss-rdbms",
            "--output",
            "json",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => AccessTokenError::CliNotInstalled,
            _ => AccessTokenError::SignInRequired,
        })?;
    let stdout = drain(child.stdout.take());
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < COMMAND_DEADLINE => thread::sleep(POLL_INTERVAL),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AccessTokenError::SignInRequired);
            }
        }
    };
    Ok(CommandOutput {
        success: status.success(),
        stdout: stdout.join().unwrap_or_default(),
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buffer = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut buffer);
        }
        buffer
    })
}

fn parse_expiry(text: &str) -> Option<u64> {
    let (date, time) = text.trim().split_once([' ', 'T'])?;
    let mut date = date.split('-');
    let year: i64 = date.next()?.parse().ok()?;
    let month: i64 = date.next()?.parse().ok()?;
    let day: i64 = date.next()?.parse().ok()?;
    let time = time.split(['.', '+', 'Z']).next()?;
    let mut time = time.split(':');
    let hour: i64 = time.next()?.parse().ok()?;
    let minute: i64 = time.next()?.parse().ok()?;
    let second: i64 = time.next().unwrap_or("0").parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = shifted_year.div_euclid(400);
    let year_of_era = shifted_year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(days * 86_400 + hour * 3_600 + minute * 60 + second).ok()
}

fn find_on_path(path: Option<OsString>) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["az.cmd", "az.exe", "az.bat"]
    } else {
        &["az"]
    };
    std::env::split_paths(&path?).find_map(|directory| {
        names
            .iter()
            .map(|name| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

#[cfg(all(test, unix))]
mod integration {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    fn fake_az(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("az");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn fetch_with(body: &str) -> Result<FetchedToken, AccessTokenError> {
        let dir = tempfile::tempdir().unwrap();
        let path = fake_az(dir.path(), body);
        AzureCli::new(Some(path)).fetch()
    }

    #[test]
    fn un_jeton_valide_est_lu_avec_son_expiration_en_secondes_epoch() {
        let fetched = fetch_with(
            r#"[ "$*" = "account get-access-token --resource-type oss-rdbms --output json" ] || exit 9
echo '{"accessToken":"tok-abc","expires_on":1900000000,"expiresOn":"2030-03-17 17:46:40.000000"}'"#,
        )
        .unwrap();
        assert_eq!(fetched.token.expose(), "tok-abc");
        assert_eq!(fetched.expires_at, 1_900_000_000);
    }

    #[test]
    fn l_expiration_retombe_sur_le_texte_expires_on_lu_comme_utc_quand_la_valeur_en_secondes_est_absente()
     {
        let fetched = fetch_with(
            r#"echo '{"accessToken":"tok-abc","expiresOn":"2030-03-17 17:46:40.000000"}'"#,
        )
        .unwrap();
        assert_eq!(fetched.expires_at, 1_900_000_000);
    }

    #[test]
    fn un_jeton_deja_expire_est_rendu_avec_son_expiration_passee() {
        let fetched = fetch_with(r#"echo '{"accessToken":"tok-old","expires_on":1000}'"#).unwrap();
        assert_eq!(fetched.expires_at, 1_000);
    }

    #[test]
    fn un_echec_de_az_ne_laisse_fuiter_aucune_sortie_d_erreur() {
        let error = fetch_with(
            "echo \"ERROR: Please run 'az login' to setup account.\" >&2\necho 'second line with tok-leak' >&2\necho '{\"accessToken\":\"tok-leak\"}'\nexit 1",
        )
        .err()
        .unwrap();
        assert_eq!(error, AccessTokenError::SignInRequired);
        let shown = error.to_string();
        assert!(shown.contains("az login"), "{shown}");
        assert!(!shown.contains("ERROR"), "{shown}");
        assert!(!shown.contains("second line"), "{shown}");
        assert!(!shown.contains("tok-leak"), "{shown}");
    }

    #[test]
    fn une_sortie_illisible_ne_fuit_pas_son_contenu() {
        let error = fetch_with("echo 'not json tok-leak'").err().unwrap();
        assert_eq!(error, AccessTokenError::Unreadable);
        assert!(!error.to_string().contains("tok-leak"));
    }

    #[test]
    fn az_est_cherche_dans_les_repertoires_du_path_dans_l_ordre() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let expected = fake_az(second.path(), "exit 0");
        let path = std::env::join_paths([first.path(), second.path()]).unwrap();
        assert_eq!(find_on_path(Some(path)), Some(expected));
        assert_eq!(find_on_path(None), None);
        assert_eq!(
            find_on_path(Some(first.path().as_os_str().to_os_string())),
            None
        );
    }

    #[test]
    fn un_az_absent_est_signale_comme_non_installe() {
        let error = AzureCli::new(None).fetch().err().unwrap();
        assert_eq!(error, AccessTokenError::CliNotInstalled);
        let dir = tempfile::tempdir().unwrap();
        let error = AzureCli::new(Some(dir.path().join("nope")))
            .fetch()
            .err()
            .unwrap();
        assert_eq!(error, AccessTokenError::CliNotInstalled);
    }
}
