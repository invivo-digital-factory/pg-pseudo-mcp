use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::contexts::connections::infrastructure::data_sources::bounded_process::{
    BoundedFailure, run_bounded,
};

const COMMAND_DEADLINE: Duration = Duration::from_secs(5);
const MAX_OUTPUT: usize = 1024 * 1024;

pub struct Target<'a> {
    pub host: &'a str,
    pub user: Option<&'a str>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DumpFailure {
    NotInstalled,
    Status(i32),
    Other,
}

pub trait ConfigDump: Send + Sync {
    fn dump(&self, target: &Target<'_>) -> Result<String, DumpFailure>;
}

pub struct SshCommand {
    command: Option<PathBuf>,
    config: Option<PathBuf>,
    deadline: Duration,
}

impl SshCommand {
    pub fn new(command: Option<PathBuf>, config: Option<PathBuf>) -> Self {
        Self {
            command,
            config,
            deadline: COMMAND_DEADLINE,
        }
    }

    pub fn from_path(config: Option<PathBuf>) -> Self {
        Self::new(find_on_path(std::env::var_os("PATH")), config)
    }

    #[cfg(test)]
    fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }
}

impl ConfigDump for SshCommand {
    fn dump(&self, target: &Target<'_>) -> Result<String, DumpFailure> {
        let command = self.command.as_ref().ok_or(DumpFailure::NotInstalled)?;
        let mut process = Command::new(command);
        process.arg("-G");
        if let Some(config) = &self.config {
            process.arg("-F").arg(config);
        }
        if let Some(user) = target.user {
            process.arg("-l").arg(user);
        }
        if let Some(port) = target.port {
            process.arg("-p").arg(port.to_string());
        }
        process.arg(target.host);
        let output =
            run_bounded(&mut process, self.deadline, MAX_OUTPUT).map_err(
                |failure| match failure {
                    BoundedFailure::NotFound => DumpFailure::NotInstalled,
                    _ => DumpFailure::Other,
                },
            )?;
        if !output.status.success() {
            return Err(output
                .status
                .code()
                .map_or(DumpFailure::Other, DumpFailure::Status));
        }
        String::from_utf8(output.stdout).map_err(|_| DumpFailure::Other)
    }
}

fn find_on_path(path: Option<OsString>) -> Option<PathBuf> {
    let name = if cfg!(windows) { "ssh.exe" } else { "ssh" };
    std::env::split_paths(&path?)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(all(test, unix))]
mod integration {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Instant;

    use super::*;

    fn fake_ssh(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("ssh");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn target(host: &str) -> Target<'_> {
        Target {
            host,
            user: None,
            port: None,
        }
    }

    #[test]
    fn la_commande_recoit_g_le_fichier_de_configuration_l_utilisateur_le_port_et_l_hote() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(dir.path(), "echo \"$*\"");
        let command = SshCommand::new(Some(ssh), Some(PathBuf::from("/cfg/ssh_config")));
        let output = command
            .dump(&Target {
                host: "jump",
                user: Some("alice"),
                port: Some(2200),
            })
            .unwrap();
        assert_eq!(output.trim(), "-G -F /cfg/ssh_config -l alice -p 2200 jump");
        let bare = SshCommand::new(Some(dir.path().join("ssh")), None);
        assert_eq!(bare.dump(&target("alias")).unwrap().trim(), "-G alias");
    }

    #[test]
    fn ssh_absent_du_path_est_signale_comme_non_installe() {
        assert_eq!(
            SshCommand::new(None, None).dump(&target("alias")),
            Err(DumpFailure::NotInstalled)
        );
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            SshCommand::new(Some(dir.path().join("nope")), None).dump(&target("alias")),
            Err(DumpFailure::NotInstalled)
        );
    }

    #[test]
    fn un_echec_rend_le_statut_sans_le_texte_d_erreur() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(
            dir.path(),
            "echo 'Could not resolve hostname secret-host' >&2\nexit 255",
        );
        assert_eq!(
            SshCommand::new(Some(ssh), None).dump(&target("alias")),
            Err(DumpFailure::Status(255))
        );
    }

    #[test]
    fn une_commande_trop_lente_est_interrompue() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(dir.path(), "sleep 5");
        let started = Instant::now();
        let outcome = SshCommand::new(Some(ssh), None)
            .with_deadline(Duration::from_millis(200))
            .dump(&target("alias"));
        assert_eq!(outcome, Err(DumpFailure::Other));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn un_descendant_en_arriere_plan_qui_garde_la_sortie_ouverte_respecte_le_delai() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(dir.path(), "sleep 30 &\necho 'hostname x'");
        let started = Instant::now();
        let outcome = SshCommand::new(Some(ssh), None)
            .with_deadline(Duration::from_millis(300))
            .dump(&target("alias"));
        assert_eq!(outcome, Err(DumpFailure::Other));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn ssh_est_cherche_dans_les_repertoires_du_path_dans_l_ordre() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let expected = fake_ssh(second.path(), "exit 0");
        let path = std::env::join_paths([first.path(), second.path()]).unwrap();
        assert_eq!(find_on_path(Some(path)), Some(expected));
        assert_eq!(find_on_path(None), None);
    }
}
