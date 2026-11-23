use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::command::{ConfigDump, DumpFailure, Target};
use super::directive_policy::{PathContext, PathError, expand_path, unsupported_directive};
use super::jump_spec::parse_list;
use super::resolved_config::{ResolvedConfig, parse};
use crate::contexts::connections::domain::types::{
    ConnectionStatus, HostKeyPolicy, OpenSshKeys, SshAgent, SshAuth, SshHop,
};

const MAX_JUMP_DEPTH: usize = 8;

#[derive(Debug, Clone, Default)]
pub struct LocalEnvironment {
    pub home: Option<PathBuf>,
    pub user: Option<String>,
}

impl LocalEnvironment {
    pub fn from_process() -> Self {
        Self {
            home: directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()),
            user: std::env::var("USER")
                .or_else(|_| std::env::var("LOGNAME"))
                .ok()
                .filter(|user| !user.is_empty()),
        }
    }
}

pub struct OpenSshResolver {
    dump: Arc<dyn ConfigDump>,
    environment: LocalEnvironment,
}

fn unavailable(reason: impl Into<String>) -> ConnectionStatus {
    ConnectionStatus::Unavailable {
        reason: reason.into(),
    }
}

fn unsupported(reason: impl Into<String>) -> ConnectionStatus {
    ConnectionStatus::Unsupported {
        reason: reason.into(),
    }
}

impl OpenSshResolver {
    pub fn new(dump: Arc<dyn ConfigDump>, environment: LocalEnvironment) -> Self {
        Self { dump, environment }
    }

    pub fn resolve(&self, alias: &str) -> Result<Vec<SshHop>, ConnectionStatus> {
        if alias.is_empty() || alias.starts_with('-') || alias.contains(char::is_whitespace) {
            return Err(unavailable("the SSH alias is not a valid host name"));
        }
        let target = Target {
            host: alias,
            user: None,
            port: None,
        };
        self.chain(&target, alias, 0, &mut Vec::new())
    }

    fn chain(
        &self,
        target: &Target<'_>,
        label: &str,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Result<Vec<SshHop>, ConnectionStatus> {
        if depth > MAX_JUMP_DEPTH {
            return Err(unsupported(format!(
                "the ProxyJump chain is longer than {MAX_JUMP_DEPTH} hops"
            )));
        }
        let key = format!("{}|{:?}|{:?}", target.host, target.user, target.port);
        if stack.contains(&key) {
            return Err(unsupported(format!(
                "the ProxyJump chain loops back on itself at hop {}",
                depth + 1
            )));
        }
        stack.push(key);
        let hops = self.chain_inner(target, label, depth, stack);
        stack.pop();
        hops
    }

    fn chain_inner(
        &self,
        target: &Target<'_>,
        label: &str,
        depth: usize,
        stack: &mut Vec<String>,
    ) -> Result<Vec<SshHop>, ConnectionStatus> {
        let config = self.configuration(target)?;
        let jumps = match &config.proxy_jump {
            Some(value) => parse_list(value)
                .map_err(|()| unsupported("the ProxyJump value cannot be understood"))?,
            None => Vec::new(),
        };
        let mut hops = Vec::new();
        for (index, jump) in jumps.iter().enumerate() {
            let jump_target = Target {
                host: &jump.host,
                user: jump.user.as_deref(),
                port: jump.port,
            };
            if index == 0 {
                hops.extend(self.chain(&jump_target, &jump.host, depth + 1, stack)?);
            } else {
                let configuration = self.configuration(&jump_target)?;
                hops.push(self.hop(&configuration, &jump.host)?);
            }
        }
        hops.push(self.hop(&config, label)?);
        Ok(hops)
    }

    fn configuration(&self, target: &Target<'_>) -> Result<ResolvedConfig, ConnectionStatus> {
        let output = self.dump.dump(target).map_err(|failure| match failure {
            DumpFailure::NotInstalled => unavailable(
                "the OpenSSH client (ssh) was not found on PATH; it is needed to resolve SSH aliases",
            ),
            DumpFailure::Status(code) => unavailable(format!(
                "ssh -G could not resolve the SSH alias (exit status {code})"
            )),
            DumpFailure::Other => unavailable("ssh -G could not resolve the SSH alias"),
        })?;
        let config = parse(&output)
            .map_err(|()| unavailable("ssh -G gave an answer the server cannot use"))?;
        match unsupported_directive(&config.directives) {
            Some(name) => Err(unsupported(format!(
                "the SSH directive {name} is not supported for SSH aliases"
            ))),
            None => Ok(config),
        }
    }

    fn hop(&self, config: &ResolvedConfig, label: &str) -> Result<SshHop, ConnectionStatus> {
        let context = PathContext {
            home: self.environment.home.as_deref(),
            local_user: self.environment.user.as_deref(),
            hostname: &config.hostname,
            port: config.port,
            remote_user: &config.user,
        };
        let expand = |directive: &str, template: &str| {
            expand_path(template, &context).map_err(|error| match error {
                PathError::Token(token) => unsupported(format!(
                    "the SSH directive {directive} uses the unsupported token %{token}"
                )),
                PathError::NoHome | PathError::NoLocalUser => unavailable(format!(
                    "the SSH directive {directive} cannot be expanded: no home directory or user name"
                )),
            })
        };
        let mut identity_files = Vec::new();
        for template in &config.identity_files {
            let path = expand("IdentityFile", template)?;
            if path.is_file() {
                identity_files.push(path);
            }
        }
        let mut known_hosts = Vec::new();
        for template in config
            .user_known_hosts
            .iter()
            .chain(&config.global_known_hosts)
        {
            known_hosts.push(expand("UserKnownHostsFile", template)?);
        }
        let agent = match config.identity_agent.as_deref() {
            None | Some("SSH_AUTH_SOCK") => SshAgent::Environment,
            Some(value) if value.eq_ignore_ascii_case("none") => SshAgent::Disabled,
            Some(value) => match value.strip_prefix('$') {
                Some(variable) => std::env::var_os(variable)
                    .map(|path| SshAgent::Socket(Path::new(&path).to_path_buf()))
                    .unwrap_or(SshAgent::Disabled),
                None => SshAgent::Socket(expand("IdentityAgent", value)?),
            },
        };
        Ok(SshHop {
            label: label.to_string(),
            host: config.hostname.clone(),
            port: config.port,
            user: config.user.clone(),
            auth: SshAuth::OpenSsh(OpenSshKeys {
                identity_files,
                agent,
                identities_only: config.identities_only,
            }),
            host_key: HostKeyPolicy::Verify {
                known_hosts,
                alias: config.host_key_alias.clone(),
            },
        })
    }
}

#[cfg(test)]
mod unit {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use super::*;

    type Call = (String, Option<String>, Option<u16>);

    #[derive(Default)]
    struct FakeDump {
        answers: HashMap<String, String>,
        calls: Mutex<Vec<Call>>,
    }

    impl FakeDump {
        fn with(mut self, host: &str, answer: &str) -> Self {
            self.answers.insert(host.to_string(), answer.to_string());
            self
        }
    }

    impl ConfigDump for FakeDump {
        fn dump(&self, target: &Target<'_>) -> Result<String, DumpFailure> {
            self.calls.lock().unwrap().push((
                target.host.to_string(),
                target.user.map(str::to_string),
                target.port,
            ));
            self.answers
                .get(target.host)
                .cloned()
                .ok_or(DumpFailure::Status(255))
        }
    }

    fn environment() -> LocalEnvironment {
        LocalEnvironment {
            home: Some(PathBuf::from("/home/me")),
            user: Some("me".to_string()),
        }
    }

    fn resolver(dump: FakeDump) -> OpenSshResolver {
        OpenSshResolver::new(Arc::new(dump), environment())
    }

    fn answer(hostname: &str, port: u16, user: &str, extra: &str) -> String {
        format!("user {user}\nhostname {hostname}\nport {port}\n{extra}")
    }

    fn hosts_of(hops: &[SshHop]) -> Vec<String> {
        hops.iter().map(|hop| hop.host.clone()).collect()
    }

    fn keys_of(hop: &SshHop) -> &OpenSshKeys {
        match &hop.auth {
            SshAuth::OpenSsh(keys) => keys,
            other => panic!("expected OpenSSH keys, got {other:?}"),
        }
    }

    #[test]
    fn un_alias_donne_un_saut_avec_les_seuls_fichiers_d_identite_existants_dans_l_ordre() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("id_first");
        let second = dir.path().join("id_second");
        std::fs::write(&first, "k").unwrap();
        std::fs::write(&second, "k").unwrap();
        let missing = dir.path().join("id_missing");
        let extra = format!(
            "identityfile {}\nidentityfile {}\nidentityfile {}\nidentitiesonly yes\n",
            first.display(),
            missing.display(),
            second.display()
        );
        let hops = resolver(
            FakeDump::default().with("bastion", &answer("bastion.example", 2222, "alice", &extra)),
        )
        .resolve("bastion")
        .unwrap();
        assert_eq!(hops.len(), 1);
        let hop = &hops[0];
        assert_eq!(
            (
                hop.label.as_str(),
                hop.host.as_str(),
                hop.port,
                hop.user.as_str()
            ),
            ("bastion", "bastion.example", 2222, "alice")
        );
        let keys = keys_of(hop);
        assert_eq!(keys.identity_files, vec![first, second]);
        assert!(keys.identities_only);
        assert_eq!(keys.agent, SshAgent::Environment);
    }

    #[test]
    fn une_liste_proxyjump_donne_les_sauts_dans_l_ordre_puis_la_cible() {
        let dump = FakeDump::default()
            .with(
                "app",
                &answer("app.real", 22, "alice", "proxyjump jump@j1:2200,j2\n"),
            )
            .with("j1", &answer("j1.real", 2200, "jump", ""))
            .with("j2", &answer("j2.real", 22, "alice", ""));
        let resolver = resolver(dump);
        let hops = resolver.resolve("app").unwrap();
        assert_eq!(hosts_of(&hops), vec!["j1.real", "j2.real", "app.real"]);
        assert_eq!(
            hops.iter()
                .map(|hop| hop.label.as_str())
                .collect::<Vec<_>>(),
            vec!["j1", "j2", "app"]
        );
        assert_eq!((hops[0].port, hops[0].user.as_str()), (2200, "jump"));
    }

    #[test]
    fn les_options_du_saut_sont_transmises_a_ssh_g() {
        let dump = Arc::new(
            FakeDump::default()
                .with(
                    "app",
                    &answer("app.real", 22, "alice", "proxyjump jump@j1:2200\n"),
                )
                .with("j1", &answer("j1.real", 2200, "jump", "")),
        );
        OpenSshResolver::new(dump.clone(), environment())
            .resolve("app")
            .unwrap();
        let calls = dump.calls.lock().unwrap().clone();
        assert!(calls.contains(&("j1".to_string(), Some("jump".to_string()), Some(2200))));
        assert!(calls.contains(&("app".to_string(), None, None)));
    }

    #[test]
    fn le_proxyjump_propre_au_premier_saut_est_suivi_mais_pas_celui_des_suivants() {
        let dump = FakeDump::default()
            .with("app", &answer("app.real", 22, "a", "proxyjump j1,j2\n"))
            .with("j1", &answer("j1.real", 22, "a", "proxyjump root\n"))
            .with("root", &answer("root.real", 22, "a", ""))
            .with("j2", &answer("j2.real", 22, "a", "proxyjump ignored\n"));
        let hops = resolver(dump).resolve("app").unwrap();
        assert_eq!(
            hosts_of(&hops),
            vec!["root.real", "j1.real", "j2.real", "app.real"]
        );
    }

    #[test]
    fn un_cycle_de_proxyjump_est_non_pris_en_charge_sans_nommer_d_hote() {
        let dump = FakeDump::default()
            .with("a", &answer("secret-a.corp", 22, "carol", "proxyjump b\n"))
            .with("b", &answer("secret-b.corp", 22, "dave", "proxyjump a\n"));
        let status = resolver(dump).resolve("a").unwrap_err();
        let ConnectionStatus::Unsupported { reason } = status else {
            panic!("a loop is unsupported");
        };
        assert!(reason.contains("hop 3"), "{reason}");
        for leaked in ["secret", "carol", "dave", "corp"] {
            assert!(!reason.contains(leaked), "{reason}");
        }
    }

    #[test]
    fn une_chaine_de_huit_sauts_passe_et_neuf_sauts_sont_refuses() {
        fn chain(jumps: usize) -> FakeDump {
            let mut dump =
                FakeDump::default().with("app", &answer("app.real", 22, "a", "proxyjump j1\n"));
            for index in 1..=jumps {
                let extra = if index < jumps {
                    format!("proxyjump j{}\n", index + 1)
                } else {
                    String::new()
                };
                dump = dump.with(
                    &format!("j{index}"),
                    &answer(&format!("j{index}.real"), 22, "a", &extra),
                );
            }
            dump
        }
        assert_eq!(resolver(chain(8)).resolve("app").unwrap().len(), 9);
        let status = resolver(chain(9)).resolve("app").unwrap_err();
        assert!(
            matches!(&status, ConnectionStatus::Unsupported { reason } if reason.contains("longer")),
            "{status:?}"
        );
    }

    #[test]
    fn proxycommand_est_non_pris_en_charge_et_nomme_mais_none_est_accepte() {
        let refused = resolver(FakeDump::default().with(
            "app",
            &answer("app.real", 22, "a", "proxycommand nc %h %p\n"),
        ))
        .resolve("app")
        .unwrap_err();
        assert!(
            matches!(&refused, ConnectionStatus::Unsupported { reason } if reason.contains("ProxyCommand")),
            "{refused:?}"
        );
        let accepted = resolver(
            FakeDump::default().with("app", &answer("app.real", 22, "a", "proxycommand none\n")),
        )
        .resolve("app");
        assert!(accepted.is_ok());
    }

    #[test]
    fn les_jetons_d_un_fichier_d_identite_sont_developpes_et_un_jeton_inconnu_est_refuse() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        std::fs::write(home.join(".ssh").join("k_app.real"), "k").unwrap();
        let environment = LocalEnvironment {
            home: Some(home.clone()),
            user: Some("me".to_string()),
        };
        let hops = OpenSshResolver::new(
            Arc::new(FakeDump::default().with(
                "app",
                &answer("app.real", 22, "a", "identityfile %d/.ssh/k_%h\n"),
            )),
            environment.clone(),
        )
        .resolve("app")
        .unwrap();
        assert_eq!(
            keys_of(&hops[0]).identity_files,
            vec![home.join(".ssh").join("k_app.real")]
        );
        let refused = OpenSshResolver::new(
            Arc::new(
                FakeDump::default().with("app", &answer("app.real", 22, "a", "identityfile %C\n")),
            ),
            environment,
        )
        .resolve("app")
        .unwrap_err();
        assert!(
            matches!(&refused, ConnectionStatus::Unsupported { reason } if reason.contains("%C")),
            "{refused:?}"
        );
    }

    #[test]
    fn stricthostkeychecking_no_ne_change_pas_la_verification_du_saut() {
        let hops = resolver(FakeDump::default().with(
            "app",
            &answer(
                "app.real",
                22,
                "a",
                "stricthostkeychecking no\nupdatehostkeys no\n",
            ),
        ))
        .resolve("app")
        .unwrap();
        assert!(matches!(&hops[0].host_key, HostKeyPolicy::Verify { .. }));
    }

    #[test]
    fn les_fichiers_known_hosts_et_l_alias_de_cle_d_hote_viennent_de_ssh_g() {
        let extra = "userknownhostsfile ~/.ssh/known_hosts\nglobalknownhostsfile /etc/ssh/ssh_known_hosts\nhostkeyalias vault\n";
        let hops = resolver(FakeDump::default().with("app", &answer("app.real", 22, "a", extra)))
            .resolve("app")
            .unwrap();
        assert_eq!(
            hops[0].host_key,
            HostKeyPolicy::Verify {
                known_hosts: vec![
                    PathBuf::from("/home/me/.ssh/known_hosts"),
                    PathBuf::from("/etc/ssh/ssh_known_hosts")
                ],
                alias: Some("vault".to_string()),
            }
        );
    }

    #[test]
    fn identityagent_none_ou_un_chemin_choisit_l_agent() {
        let none = resolver(
            FakeDump::default().with("app", &answer("app.real", 22, "a", "identityagent none\n")),
        )
        .resolve("app")
        .unwrap();
        assert_eq!(keys_of(&none[0]).agent, SshAgent::Disabled);
        let socket = resolver(FakeDump::default().with(
            "app",
            &answer("app.real", 22, "a", "identityagent ~/agent.sock\n"),
        ))
        .resolve("app")
        .unwrap();
        assert_eq!(
            keys_of(&socket[0]).agent,
            SshAgent::Socket(PathBuf::from("/home/me/agent.sock"))
        );
    }

    #[test]
    fn un_echec_de_ssh_g_rend_un_motif_fixe_avec_le_statut() {
        let status = resolver(FakeDump::default())
            .resolve("unknown")
            .unwrap_err();
        assert_eq!(
            status,
            ConnectionStatus::Unavailable {
                reason: "ssh -G could not resolve the SSH alias (exit status 255)".to_string()
            }
        );
    }

    #[test]
    fn un_alias_qui_ressemble_a_une_option_est_refuse_sans_appeler_ssh() {
        let dump = Arc::new(FakeDump::default());
        for alias in ["-oProxyCommand=x", "", "two words"] {
            let status = OpenSshResolver::new(dump.clone(), environment())
                .resolve(alias)
                .unwrap_err();
            assert!(
                matches!(status, ConnectionStatus::Unavailable { .. }),
                "{alias}"
            );
        }
        assert!(dump.calls.lock().unwrap().is_empty());
    }
}
