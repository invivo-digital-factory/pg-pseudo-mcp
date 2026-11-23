use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::Deserialize;

use super::{AccessTokenFetcher, FetchedToken};
use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::types::Secret;
use crate::contexts::connections::infrastructure::data_sources::bounded_process::{
    BoundedFailure, run_bounded,
};

const COMMAND_DEADLINE: Duration = Duration::from_secs(30);
const MAX_OUTPUT: usize = 1024 * 1024;

pub struct AzureCli {
    command: Option<PathBuf>,
    deadline: Duration,
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
        Self {
            command,
            deadline: COMMAND_DEADLINE,
        }
    }

    pub fn from_path() -> Self {
        Self::new(find_on_path(std::env::var_os("PATH")))
    }

    #[cfg(test)]
    fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }
}

impl AccessTokenFetcher for AzureCli {
    fn fetch(&self) -> Result<FetchedToken, AccessTokenError> {
        let Some(command) = &self.command else {
            return Err(AccessTokenError::CliNotInstalled);
        };
        let output = run(command, self.deadline)?;
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

fn run(command: &Path, deadline: Duration) -> Result<CommandOutput, AccessTokenError> {
    let output = run_bounded(
        Command::new(command).args([
            "account",
            "get-access-token",
            "--resource-type",
            "oss-rdbms",
            "--output",
            "json",
        ]),
        deadline,
        MAX_OUTPUT,
    )
    .map_err(|failure| match failure {
        BoundedFailure::NotFound => AccessTokenError::CliNotInstalled,
        _ => AccessTokenError::SignInRequired,
    })?;
    Ok(CommandOutput {
        success: output.status.success(),
        stdout: output.stdout,
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
    fn un_descendant_en_arriere_plan_qui_garde_la_sortie_ouverte_respecte_le_delai() {
        let dir = tempfile::tempdir().unwrap();
        let path = fake_az(dir.path(), "sleep 30 &\necho '{}'");
        let started = std::time::Instant::now();
        let error = AzureCli::new(Some(path))
            .with_deadline(Duration::from_millis(300))
            .fetch()
            .err()
            .unwrap();
        assert_eq!(error, AccessTokenError::SignInRequired);
        assert!(started.elapsed() < Duration::from_secs(3));
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
