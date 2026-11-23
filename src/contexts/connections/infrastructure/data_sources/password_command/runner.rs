use std::process::Command;
use std::time::Duration;

use super::PasswordCommandRunner;
use crate::contexts::connections::domain::errors::PasswordCommandError;
use crate::contexts::connections::domain::types::Secret;
use crate::contexts::connections::infrastructure::data_sources::bounded_process::{
    BoundedFailure, run_bounded,
};

const COMMAND_DEADLINE: Duration = Duration::from_secs(30);
const MAX_OUTPUT: usize = 64 * 1024;

pub struct ProcessPasswordCommand {
    deadline: Duration,
}

impl ProcessPasswordCommand {
    pub fn new() -> Self {
        Self::with_deadline(COMMAND_DEADLINE)
    }

    pub fn with_deadline(deadline: Duration) -> Self {
        Self { deadline }
    }
}

impl Default for ProcessPasswordCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordCommandRunner for ProcessPasswordCommand {
    fn run(&self, argv: &[String]) -> Result<Secret, PasswordCommandError> {
        let (program, arguments) = argv
            .split_first()
            .ok_or(PasswordCommandError::CouldNotStart)?;
        let output = run_bounded(
            Command::new(program).args(arguments),
            self.deadline,
            MAX_OUTPUT,
        )
        .map_err(|failure| match failure {
            BoundedFailure::NotFound => PasswordCommandError::NotFound,
            BoundedFailure::CouldNotStart => PasswordCommandError::CouldNotStart,
            BoundedFailure::TimedOut => PasswordCommandError::TimedOut,
            BoundedFailure::OutputTooLarge => PasswordCommandError::OutputTooLarge,
        })?;
        match output.status.code() {
            Some(0) => password_from(output.stdout),
            Some(code) => Err(PasswordCommandError::ExitStatus(code)),
            None => Err(PasswordCommandError::Interrupted),
        }
    }
}

fn password_from(output: Vec<u8>) -> Result<Secret, PasswordCommandError> {
    let text = String::from_utf8(output).map_err(|_| PasswordCommandError::NotUtf8)?;
    let text = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(&text);
    if text.is_empty() {
        return Err(PasswordCommandError::NoOutput);
    }
    Ok(Secret::new(text))
}

#[cfg(all(test, unix))]
mod integration {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use super::*;

    fn script(dir: &Path, body: &str) -> Vec<String> {
        let path = dir.join("password-command");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        vec![path.to_string_lossy().into_owned()]
    }

    fn run_script(body: &str) -> Result<Secret, PasswordCommandError> {
        let dir = tempfile::tempdir().unwrap();
        ProcessPasswordCommand::new().run(&script(dir.path(), body))
    }

    #[test]
    fn une_sortie_terminee_par_un_saut_de_ligne_perd_ce_saut_de_ligne() {
        assert_eq!(run_script("printf 's3cret\\n'").unwrap().expose(), "s3cret");
        assert_eq!(
            run_script("printf 's3cret\\r\\n'").unwrap().expose(),
            "s3cret"
        );
        assert_eq!(run_script("printf 's3cret'").unwrap().expose(), "s3cret");
    }

    #[test]
    fn un_seul_saut_de_ligne_final_est_retire() {
        assert_eq!(run_script("printf 'a b\\n\\n'").unwrap().expose(), "a b\n");
    }

    #[test]
    fn un_code_de_sortie_non_nul_rend_le_statut_sans_aucune_sortie_de_la_commande() {
        let error =
            run_script("echo 'out-leak-4411'\necho 'err-leak-4411' >&2\nexit 44").unwrap_err();
        assert_eq!(error, PasswordCommandError::ExitStatus(44));
        let shown = error.to_string();
        assert!(shown.contains("status 44"), "{shown}");
        assert!(!shown.contains("leak"), "{shown}");
    }

    #[test]
    fn la_sortie_d_erreur_n_atteint_jamais_le_mot_de_passe() {
        let secret = run_script("echo 'err-text' >&2\nprintf 'pw'").unwrap();
        assert_eq!(secret.expose(), "pw");
    }

    #[test]
    fn un_programme_absent_est_signale_comme_introuvable() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope").to_string_lossy().into_owned();
        assert_eq!(
            ProcessPasswordCommand::new().run(&[missing]),
            Err(PasswordCommandError::NotFound)
        );
    }

    #[test]
    fn un_fichier_non_executable_ne_peut_pas_etre_demarre() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plain");
        std::fs::write(&path, "data").unwrap();
        assert_eq!(
            ProcessPasswordCommand::new().run(&[path.to_string_lossy().into_owned()]),
            Err(PasswordCommandError::CouldNotStart)
        );
    }

    #[test]
    fn une_commande_vide_ne_peut_pas_etre_demarree() {
        assert_eq!(
            ProcessPasswordCommand::new().run(&[]),
            Err(PasswordCommandError::CouldNotStart)
        );
    }

    #[test]
    fn une_commande_trop_lente_est_interrompue_apres_le_delai() {
        let dir = tempfile::tempdir().unwrap();
        let argv = script(dir.path(), "sleep 5");
        let started = std::time::Instant::now();
        let error = ProcessPasswordCommand::with_deadline(Duration::from_millis(200))
            .run(&argv)
            .unwrap_err();
        assert_eq!(error, PasswordCommandError::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn un_descendant_en_arriere_plan_qui_garde_la_sortie_ouverte_respecte_le_delai() {
        let dir = tempfile::tempdir().unwrap();
        let argv = script(dir.path(), "sleep 30 &\nprintf 'pw'");
        let started = std::time::Instant::now();
        let error = ProcessPasswordCommand::with_deadline(Duration::from_millis(300))
            .run(&argv)
            .unwrap_err();
        assert_eq!(error, PasswordCommandError::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn une_sortie_vide_ou_non_utf8_est_refusee() {
        assert_eq!(run_script("printf ''"), Err(PasswordCommandError::NoOutput));
        assert_eq!(
            run_script("printf '\\n'"),
            Err(PasswordCommandError::NoOutput)
        );
        assert_eq!(
            run_script("printf '\\377\\376'"),
            Err(PasswordCommandError::NotUtf8)
        );
    }

    #[test]
    fn les_arguments_sont_passes_sans_shell() {
        let dir = tempfile::tempdir().unwrap();
        let mut argv = script(dir.path(), "printf '%s|' \"$@\"");
        argv.extend(["a b".to_string(), "$HOME".to_string(), "c;d".to_string()]);
        let secret = ProcessPasswordCommand::new().run(&argv).unwrap();
        assert_eq!(secret.expose(), "a b|$HOME|c;d|");
    }
}
