use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::command::{ConfigDump, Target};
use super::resolved_config::{parse, words};
use crate::contexts::connections::domain::repositories::SshAliasDirectory;

const MAX_INCLUDE_DEPTH: usize = 4;

pub struct OpenSshAliasDirectory {
    dump: Arc<dyn ConfigDump>,
    config: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl OpenSshAliasDirectory {
    pub fn new(dump: Arc<dyn ConfigDump>, config: Option<PathBuf>, home: Option<PathBuf>) -> Self {
        Self { dump, config, home }
    }

    fn ssh_directory(&self) -> Option<PathBuf> {
        self.home.as_ref().map(|home| home.join(".ssh"))
    }

    fn root_config(&self) -> Option<PathBuf> {
        self.config
            .clone()
            .or_else(|| self.ssh_directory().map(|dir| dir.join("config")))
    }

    fn collect(&self, file: &Path, depth: usize, found: &mut Vec<Vec<String>>) {
        let Ok(contents) = std::fs::read_to_string(file) else {
            return;
        };
        for line in contents.lines() {
            let (keyword, arguments) = split_directive(line);
            let arguments: Vec<String> = arguments
                .into_iter()
                .take_while(|word| !word.starts_with('#'))
                .collect();
            if keyword.eq_ignore_ascii_case("host") {
                found.push(arguments);
            } else if keyword.eq_ignore_ascii_case("include") && depth < MAX_INCLUDE_DEPTH {
                for pattern in arguments {
                    for included in self.included_files(&pattern) {
                        self.collect(&included, depth + 1, found);
                    }
                }
            }
        }
    }

    fn host_lines(&self) -> Vec<Vec<String>> {
        let mut lines = Vec::new();
        if let Some(root) = self.root_config() {
            self.collect(&root, 0, &mut lines);
        }
        lines
    }

    fn included_files(&self, pattern: &str) -> Vec<PathBuf> {
        let expanded = match pattern.strip_prefix("~/") {
            Some(rest) => self.home.as_ref().map(|home| home.join(rest)),
            None => Some(PathBuf::from(pattern)),
        };
        let Some(expanded) = expanded else {
            return Vec::new();
        };
        let path = if expanded.is_absolute() {
            expanded
        } else {
            match self.ssh_directory() {
                Some(directory) => directory.join(expanded),
                None => return Vec::new(),
            }
        };
        let (Some(directory), Some(name)) = (path.parent(), path.file_name()) else {
            return Vec::new();
        };
        let name = name.to_string_lossy();
        if !name.contains(['*', '?']) {
            return vec![path];
        }
        let mut matches: Vec<PathBuf> = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| wildcard_match(&name, &entry.file_name().to_string_lossy()))
            .map(|entry| entry.path())
            .collect();
        matches.sort();
        matches
    }
}

impl SshAliasDirectory for OpenSshAliasDirectory {
    fn alias_for(&self, host: &str, port: u16, user: &str) -> Option<String> {
        self.known_aliases().into_iter().find(|alias| {
            let target = Target {
                host: alias,
                user: None,
                port: None,
            };
            self.dump
                .dump(&target)
                .ok()
                .and_then(|output| parse(&output).ok())
                .is_some_and(|config| {
                    config.hostname == host && config.port == port && config.user == user
                })
        })
    }

    fn known_aliases(&self) -> Vec<String> {
        let mut found: Vec<String> = Vec::new();
        for pattern in self.host_lines().into_iter().flatten() {
            let plain = !pattern.contains(['*', '?', '!']);
            if plain && !found.contains(&pattern) {
                found.push(pattern);
            }
        }
        found
    }

    fn defines(&self, alias: &str) -> bool {
        self.root_config().is_none()
            || self
                .host_lines()
                .iter()
                .any(|patterns| host_line_matches(patterns, alias))
    }
}

fn host_line_matches(patterns: &[String], alias: &str) -> bool {
    let excluded = patterns
        .iter()
        .filter_map(|pattern| pattern.strip_prefix('!'))
        .any(|pattern| wildcard_match(pattern, alias));
    !excluded
        && patterns.iter().any(|pattern| {
            !pattern.starts_with('!')
                && !pattern.chars().all(|character| character == '*')
                && wildcard_match(pattern, alias)
        })
}

fn split_directive(line: &str) -> (&str, Vec<String>) {
    let line = line.trim();
    let end = line
        .find(|character: char| character.is_whitespace() || character == '=')
        .unwrap_or(line.len());
    let (keyword, rest) = line.split_at(end);
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=').unwrap_or(rest);
    (keyword, words(rest))
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    match pattern.chars().next() {
        None => text.is_empty(),
        Some('*') => {
            let rest = &pattern[1..];
            (0..=text.len())
                .filter(|index| text.is_char_boundary(*index))
                .any(|index| wildcard_match(rest, &text[index..]))
        }
        Some(expected) => {
            let mut characters = text.chars();
            match characters.next() {
                Some(actual) if expected == '?' || expected == actual => {
                    wildcard_match(&pattern[expected.len_utf8()..], characters.as_str())
                }
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod unit {
    use std::collections::HashMap;

    use super::super::command::DumpFailure;
    use super::*;

    struct Answers(HashMap<String, String>);

    impl ConfigDump for Answers {
        fn dump(&self, target: &Target<'_>) -> Result<String, DumpFailure> {
            self.0
                .get(target.host)
                .cloned()
                .ok_or(DumpFailure::Status(255))
        }
    }

    fn answers(entries: &[(&str, &str, u16, &str)]) -> Arc<Answers> {
        Arc::new(Answers(
            entries
                .iter()
                .map(|(alias, host, port, user)| {
                    (
                        alias.to_string(),
                        format!("user {user}\nhostname {host}\nport {port}\n"),
                    )
                })
                .collect(),
        ))
    }

    fn directory(
        config: &str,
        entries: &[(&str, &str, u16, &str)],
    ) -> (tempfile::TempDir, OpenSshAliasDirectory) {
        let home = tempfile::tempdir().unwrap();
        let ssh = home.path().join(".ssh");
        std::fs::create_dir_all(ssh.join("conf.d")).unwrap();
        std::fs::write(ssh.join("config"), config).unwrap();
        let directory =
            OpenSshAliasDirectory::new(answers(entries), None, Some(home.path().to_path_buf()));
        (home, directory)
    }

    #[test]
    fn un_alias_dont_la_resolution_correspond_au_tunnel_est_reutilise() {
        let (_home, directory) = directory(
            "Host other\n  HostName x\nHost corp-bastion\n  HostName bastion.corp\n",
            &[
                ("other", "other.corp", 22, "jump"),
                ("corp-bastion", "bastion.corp", 22, "jump"),
            ],
        );
        assert_eq!(
            directory.alias_for("bastion.corp", 22, "jump").as_deref(),
            Some("corp-bastion")
        );
        assert_eq!(directory.alias_for("bastion.corp", 2222, "jump"), None);
        assert_eq!(directory.alias_for("bastion.corp", 22, "someone"), None);
    }

    #[test]
    fn les_motifs_a_jokers_et_les_negations_ne_sont_pas_des_candidats() {
        let (_home, directory) = directory(
            "Host *\n  User x\nHost web-* !web-old\nHost plain  # comment\n",
            &[("plain", "plain.corp", 22, "jump")],
        );
        assert_eq!(directory.known_aliases(), vec!["plain"]);
    }

    #[test]
    fn les_fichiers_inclus_sont_parcourus_avec_leurs_jokers() {
        let (home, directory) = directory(
            "Include conf.d/*.conf\nHost first\n",
            &[("from-include", "inc.corp", 22, "jump")],
        );
        std::fs::write(
            home.path().join(".ssh/conf.d/team.conf"),
            "Host from-include\n",
        )
        .unwrap();
        std::fs::write(home.path().join(".ssh/conf.d/skip.txt"), "Host skipped\n").unwrap();
        assert_eq!(directory.known_aliases(), vec!["from-include", "first"]);
        assert_eq!(
            directory.alias_for("inc.corp", 22, "jump").as_deref(),
            Some("from-include")
        );
    }

    #[test]
    fn sans_fichier_de_configuration_aucun_alias_n_est_connu() {
        let home = tempfile::tempdir().unwrap();
        let directory =
            OpenSshAliasDirectory::new(answers(&[]), None, Some(home.path().to_path_buf()));
        assert!(directory.known_aliases().is_empty());
        assert_eq!(directory.alias_for("h", 22, "u"), None);
    }

    #[test]
    fn le_fichier_de_configuration_de_test_remplace_celui_du_dossier_personnel() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("ssh_config");
        std::fs::write(&config, "Host from-override\n").unwrap();
        let directory = OpenSshAliasDirectory::new(
            answers(&[("from-override", "o.corp", 22, "jump")]),
            Some(config),
            None,
        );
        assert_eq!(directory.known_aliases(), vec!["from-override"]);
    }

    #[test]
    fn le_joker_etoile_et_point_d_interrogation_sont_compris() {
        assert!(wildcard_match("*.conf", "team.conf"));
        assert!(!wildcard_match("*.conf", "team.txt"));
        assert!(wildcard_match("a?c", "abc"));
        assert!(!wildcard_match("a?c", "ac"));
    }

    #[test]
    fn un_alias_defini_par_un_bloc_host_exact_est_defini() {
        let (_home, directory) = directory("Host corp-bastion other\n  HostName x\n", &[]);
        assert!(directory.defines("corp-bastion"));
        assert!(directory.defines("other"));
        assert!(!directory.defines("corp-bastio"));
    }

    #[test]
    fn un_alias_qui_correspond_a_un_motif_a_jokers_est_defini() {
        let (_home, directory) = directory("Host *.corp web-?\n", &[]);
        assert!(directory.defines("db.corp"));
        assert!(directory.defines("web-1"));
        assert!(!directory.defines("web-12"));
        assert!(!directory.defines("db.example"));
    }

    #[test]
    fn un_bloc_host_etoile_seul_ne_definit_aucun_alias() {
        let (_home, directory) = directory("Host *\n  User x\nHost * !nothing\n", &[]);
        assert!(!directory.defines("anything"));
    }

    #[test]
    fn un_motif_nie_exclut_l_alias_de_sa_ligne_mais_pas_des_autres() {
        let (_home, directory) = directory("Host web-* !web-old\nHost web-old-bis\n", &[]);
        assert!(directory.defines("web-new"));
        assert!(!directory.defines("web-old"));
        assert!(directory.defines("web-old-bis"));
    }

    #[test]
    fn un_alias_defini_dans_un_fichier_inclus_est_defini() {
        let (home, directory) = directory("Include conf.d/*.conf\n", &[]);
        std::fs::write(
            home.path().join(".ssh/conf.d/team.conf"),
            "Host from-include\n",
        )
        .unwrap();
        assert!(directory.defines("from-include"));
        assert!(!directory.defines("elsewhere"));
    }

    #[test]
    fn un_bloc_match_n_est_pas_evalue_et_ne_definit_rien_par_lui_meme() {
        let (_home, directory) = directory("Match host db\n  User x\nHost real\n", &[]);
        assert!(directory.defines("real"));
        assert!(!directory.defines("db"));
    }

    #[test]
    fn sans_fichier_de_configuration_lisible_aucun_alias_n_est_defini() {
        let home = tempfile::tempdir().unwrap();
        let directory =
            OpenSshAliasDirectory::new(answers(&[]), None, Some(home.path().to_path_buf()));
        assert!(!directory.defines("any"));
    }

    #[test]
    fn sans_emplacement_de_configuration_connu_aucun_alias_n_est_refuse() {
        let directory = OpenSshAliasDirectory::new(answers(&[]), None, None);
        assert!(directory.defines("any"));
    }

    #[test]
    fn un_argument_entre_guillemets_est_lu_sans_ses_guillemets() {
        let (_home, directory) = directory("Host \"quoted\"\n", &[]);
        assert!(directory.defines("quoted"));
        assert!(!directory.defines("\"quoted\""));
    }

    #[test]
    fn la_forme_avec_signe_egal_est_lue_avec_ou_sans_espaces() {
        let (_home, directory) = directory("Host=eqform\nHost = spaced-eq\nhost =tight\n", &[]);
        assert!(directory.defines("eqform"));
        assert!(directory.defines("spaced-eq"));
        assert!(directory.defines("tight"));
    }

    #[test]
    fn un_argument_entre_guillemets_avec_espace_reste_un_seul_motif() {
        let (_home, directory) = directory("Host \"two words\" spaced\n", &[]);
        assert!(directory.defines("two words"));
        assert!(directory.defines("spaced"));
        assert!(!directory.defines("two"));
        assert!(!directory.defines("words"));
    }
}
