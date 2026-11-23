use std::process::ExitCode;

use crate::contexts::connections::domain::types::{AuthMode, ImportReport, ImportedConnection};
use crate::contexts::connections::use_cases::ImportDbeaverConnections;

pub struct ImportDbeaverHandler {
    use_case: ImportDbeaverConnections,
}

impl ImportDbeaverHandler {
    pub fn new(use_case: ImportDbeaverConnections) -> Self {
        Self { use_case }
    }

    pub fn run(&self, force: bool) -> ExitCode {
        match self.use_case.execute(force) {
            Ok(report) => {
                print!("{}", summary(&report));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        }
    }
}

pub fn summary(report: &ImportReport) -> String {
    let mut text = format!(
        "Wrote {}: {} connection(s) imported, {} skipped. No password was written.\n",
        report.path.display(),
        report.imported.len(),
        report.skipped.len()
    );
    text.push_str(
        "\nThe connection file now takes precedence over DBeaver: a connection whose SSH alias has no Host block in the OpenSSH config is listed unavailable. Add the suggested Host blocks below before using these servers, or keep the file aside until then.\n",
    );
    text.push_str("\nWhat is left to do, per imported connection:\n");
    for connection in &report.imported {
        text.push_str(&format!("  {}\n", connection.name));
        for step in steps_of(connection) {
            text.push_str(&format!("    - {step}\n"));
        }
    }
    if !report.skipped.is_empty() {
        text.push_str("\nSkipped:\n");
        for skipped in &report.skipped {
            text.push_str(&format!("  {}: {}\n", skipped.name, skipped.reason));
        }
    }
    if !report.host_blocks.is_empty() {
        text.push_str(
            "\nSuggested blocks for ~/.ssh/config (nothing was edited; add them yourself):\n",
        );
        for block in &report.host_blocks {
            text.push_str(&format!(
                "\nHost {}\n  HostName {}\n  Port {}\n  User {}\n",
                block.alias, block.host, block.port, block.user
            ));
            if let Some(identity) = &block.identity_file {
                text.push_str(&format!("  IdentityFile {}\n", identity.display()));
            }
        }
    }
    text
}

fn steps_of(connection: &ImportedConnection) -> Vec<String> {
    let mut steps = Vec::new();
    match &connection.auth {
        AuthMode::Azure => {
            steps.push("run az login: the Azure token is used as the password".to_string());
        }
        AuthMode::Pgpass | AuthMode::Command(_) => steps.push(
            "add a pgpass entry for this host, or switch auth to \"command\" with a password_command"
                .to_string(),
        ),
    }
    if let Some(alias) = &connection.ssh_alias {
        if connection.alias_proposed {
            steps.push(format!(
                "add the Host block {alias} suggested below to ~/.ssh/config"
            ));
        } else {
            steps.push(format!("ssh alias {alias} already exists in ~/.ssh/config"));
        }
        steps.push(format!(
            "make sure the host keys of {alias} (and of any jump host) are in known_hosts"
        ));
    }
    if let Some(original) = &connection.type_defaulted_from {
        steps.push(format!(
            "DBeaver type \"{original}\" is not dev, test or prod: imported as prod, edit the type if needed"
        ));
    }
    steps
}

#[cfg(test)]
mod unit {
    use std::path::PathBuf;

    use super::*;
    use crate::contexts::connections::domain::types::{HostBlock, SkippedConnection};

    fn report() -> ImportReport {
        ImportReport {
            path: PathBuf::from("/config/connections.toml"),
            imported: vec![
                ImportedConnection {
                    name: "native".into(),
                    auth: AuthMode::Pgpass,
                    ssh_alias: Some("bastion.corp".into()),
                    alias_proposed: true,
                    type_defaulted_from: Some("staging".into()),
                },
                ImportedConnection {
                    name: "shop".into(),
                    auth: AuthMode::Azure,
                    ssh_alias: Some("corp-bastion".into()),
                    alias_proposed: false,
                    type_defaulted_from: None,
                },
            ],
            skipped: vec![SkippedConnection {
                name: "legacy".into(),
                reason: "engine mysql is not supported, only PostgreSQL".into(),
            }],
            host_blocks: vec![HostBlock {
                alias: "bastion.corp".into(),
                host: "bastion.corp".into(),
                port: 2222,
                user: "jump".into(),
                identity_file: Some(PathBuf::from("/keys/id_ed25519")),
            }],
        }
    }

    #[test]
    fn le_resume_dit_ce_qu_il_reste_a_faire_par_connexion() {
        let text = summary(&report());
        assert!(
            text.contains("2 connection(s) imported, 1 skipped"),
            "{text}"
        );
        assert!(text.contains("add a pgpass entry"), "{text}");
        assert!(text.contains("run az login"), "{text}");
        assert!(
            text.contains("Host block bastion.corp suggested below"),
            "{text}"
        );
        assert!(text.contains("alias corp-bastion already exists"), "{text}");
        assert!(text.contains("known_hosts"), "{text}");
        assert!(
            text.contains("\"staging\" is not dev, test or prod"),
            "{text}"
        );
    }

    #[test]
    fn le_resume_annonce_en_tete_que_le_fichier_prime_sur_dbeaver_et_qu_il_faut_les_blocs_host() {
        let text = summary(&report());
        let notice = text.find("takes precedence over DBeaver").expect("notice");
        assert!(notice < text.find("What is left to do").unwrap(), "{text}");
        assert!(notice < text.find("Suggested blocks").unwrap(), "{text}");
        assert!(text.contains("Add the suggested Host blocks"), "{text}");
        assert!(text.contains("keep the file aside"), "{text}");
    }

    #[test]
    fn les_connexions_ignorees_sont_listees_avec_leur_raison() {
        let text = summary(&report());
        assert!(
            text.contains("legacy: engine mysql is not supported, only PostgreSQL"),
            "{text}"
        );
    }

    #[test]
    fn le_bloc_host_suggere_porte_hote_port_utilisateur_et_fichier_de_cle() {
        let text = summary(&report());
        assert!(
            text.contains(
                "Host bastion.corp\n  HostName bastion.corp\n  Port 2222\n  User jump\n  IdentityFile /keys/id_ed25519\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn sans_bloc_ni_connexion_ignoree_le_resume_n_a_pas_de_section_vide() {
        let mut quiet = report();
        quiet.skipped.clear();
        quiet.host_blocks.clear();
        let text = summary(&quiet);
        assert!(!text.contains("Skipped:"), "{text}");
        assert!(!text.contains("Suggested blocks"), "{text}");
    }
}
