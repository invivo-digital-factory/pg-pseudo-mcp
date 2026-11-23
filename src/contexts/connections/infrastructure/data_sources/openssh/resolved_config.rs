#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedConfig {
    pub hostname: String,
    pub port: u16,
    pub user: String,
    pub identity_files: Vec<String>,
    pub identities_only: bool,
    pub identity_agent: Option<String>,
    pub proxy_jump: Option<String>,
    pub user_known_hosts: Vec<String>,
    pub global_known_hosts: Vec<String>,
    pub host_key_alias: Option<String>,
    pub directives: Vec<(String, String)>,
}

pub(super) fn parse(output: &str) -> Result<ResolvedConfig, ()> {
    let mut directives = Vec::new();
    for line in output.lines() {
        if let Some((key, value)) = line.trim().split_once(char::is_whitespace) {
            directives.push((key.to_ascii_lowercase(), value.trim().to_string()));
        }
    }
    let first = |key: &str| {
        directives
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let all = |key: &str| {
        directives
            .iter()
            .filter(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
    };
    let hostname = first("hostname")
        .filter(|value| !value.is_empty())
        .ok_or(())?;
    let user = first("user").filter(|value| !value.is_empty()).ok_or(())?;
    let port = first("port")
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or(())?;
    let listed = |key: &str| {
        all(key)
            .into_iter()
            .flat_map(words)
            .collect::<Vec<String>>()
    };
    Ok(ResolvedConfig {
        hostname: hostname.to_string(),
        port,
        user: user.to_string(),
        identity_files: all("identityfile").into_iter().map(unquote).collect(),
        identities_only: first("identitiesonly")
            .is_some_and(|value| value.eq_ignore_ascii_case("yes")),
        identity_agent: first("identityagent").map(unquote),
        proxy_jump: first("proxyjump")
            .filter(|value| !value.eq_ignore_ascii_case("none"))
            .map(str::to_string),
        user_known_hosts: listed("userknownhostsfile"),
        global_known_hosts: listed("globalknownhostsfile"),
        host_key_alias: first("hostkeyalias")
            .filter(|value| !value.eq_ignore_ascii_case("none"))
            .map(unquote),
        directives,
    })
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}

fn words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in value.chars() {
        match character {
            '"' => quoted = !quoted,
            space if space.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

#[cfg(test)]
mod unit {
    use super::*;

    const DUMP: &str = "user alice\nhostname bastion.example\nport 2222\naddressfamily any\nbatchmode no\nidentityfile ~/.ssh/id_ed25519\nidentityfile ~/.ssh/id_rsa\nidentitiesonly yes\nuserknownhostsfile ~/.ssh/known_hosts ~/.ssh/known_hosts2\nglobalknownhostsfile /etc/ssh/ssh_known_hosts /etc/ssh/ssh_known_hosts2\nproxyjump jump@j1:2200,j2\nstricthostkeychecking true\nhostkeyalias vault\nidentityagent /run/agent.sock\n";

    #[test]
    fn une_sortie_ssh_g_complete_est_lue_champ_par_champ() {
        let config = parse(DUMP).unwrap();
        assert_eq!(config.user, "alice");
        assert_eq!(config.hostname, "bastion.example");
        assert_eq!(config.port, 2222);
        assert_eq!(
            config.identity_files,
            vec!["~/.ssh/id_ed25519", "~/.ssh/id_rsa"]
        );
        assert!(config.identities_only);
        assert_eq!(config.identity_agent.as_deref(), Some("/run/agent.sock"));
        assert_eq!(config.proxy_jump.as_deref(), Some("jump@j1:2200,j2"));
        assert_eq!(
            config.user_known_hosts,
            vec!["~/.ssh/known_hosts", "~/.ssh/known_hosts2"]
        );
        assert_eq!(
            config.global_known_hosts,
            vec!["/etc/ssh/ssh_known_hosts", "/etc/ssh/ssh_known_hosts2"]
        );
        assert_eq!(config.host_key_alias.as_deref(), Some("vault"));
    }

    #[test]
    fn proxyjump_none_et_les_options_absentes_prennent_leur_valeur_par_defaut() {
        let config = parse("user a\nhostname h\nport 22\nproxyjump none\n").unwrap();
        assert_eq!(config.proxy_jump, None);
        assert!(!config.identities_only);
        assert_eq!(config.identity_agent, None);
        assert_eq!(config.host_key_alias, None);
        assert!(config.identity_files.is_empty());
    }

    #[test]
    fn les_chemins_entre_guillemets_gardent_leurs_espaces() {
        let config = parse(
            "user a\nhostname h\nport 22\nidentityfile \"/Users/me/My Keys/id\"\nuserknownhostsfile \"/a b/known\" /c/known\n",
        )
        .unwrap();
        assert_eq!(config.identity_files, vec!["/Users/me/My Keys/id"]);
        assert_eq!(config.user_known_hosts, vec!["/a b/known", "/c/known"]);
    }

    #[test]
    fn toutes_les_lignes_restent_disponibles_pour_la_politique_des_directives() {
        let config = parse("user a\nhostname h\nport 22\nproxycommand nc %h %p\n").unwrap();
        assert!(
            config
                .directives
                .contains(&("proxycommand".to_string(), "nc %h %p".to_string()))
        );
    }

    #[test]
    fn une_sortie_sans_hote_ni_port_ni_utilisateur_est_refusee() {
        assert!(parse("user a\nport 22\n").is_err());
        assert!(parse("hostname h\nport 22\n").is_err());
        assert!(parse("user a\nhostname h\n").is_err());
        assert!(parse("user a\nhostname h\nport nope\n").is_err());
        assert!(parse("").is_err());
    }
}
