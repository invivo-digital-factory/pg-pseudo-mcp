#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct JumpSpec {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
}

pub(super) fn parse_list(value: &str) -> Result<Vec<JumpSpec>, ()> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    value.split(',').map(parse_one).collect()
}

fn parse_one(text: &str) -> Result<JumpSpec, ()> {
    let text = text.trim();
    let text = text.strip_prefix("ssh://").unwrap_or(text);
    let text = text.strip_suffix('/').unwrap_or(text);
    let (user, location) = match text.rsplit_once('@') {
        Some((user, location)) if !user.is_empty() => (Some(user.to_string()), location),
        Some(_) => return Err(()),
        None => (None, text),
    };
    let (host, port) = split_host_port(location)?;
    if host.is_empty()
        || host.starts_with('-')
        || user.as_deref().is_some_and(|u| u.starts_with('-'))
    {
        return Err(());
    }
    Ok(JumpSpec {
        user,
        host: host.to_string(),
        port,
    })
}

fn split_host_port(location: &str) -> Result<(&str, Option<u16>), ()> {
    if let Some(bracketed) = location.strip_prefix('[') {
        let (host, after) = bracketed.split_once(']').ok_or(())?;
        return match after {
            "" => Ok((host, None)),
            after => Ok((host, Some(parse_port(after.strip_prefix(':').ok_or(())?)?))),
        };
    }
    match location.rsplit_once(':') {
        Some((host, port)) => Ok((host, Some(parse_port(port)?))),
        None => Ok((location, None)),
    }
}

fn parse_port(text: &str) -> Result<u16, ()> {
    match text.parse::<u16>() {
        Ok(port) if port != 0 => Ok(port),
        _ => Err(()),
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    fn spec(user: Option<&str>, host: &str, port: Option<u16>) -> JumpSpec {
        JumpSpec {
            user: user.map(str::to_string),
            host: host.to_string(),
            port,
        }
    }

    #[test]
    fn une_liste_de_sauts_est_lue_dans_l_ordre_avec_utilisateur_et_port() {
        assert_eq!(
            parse_list("jump@j1:2200,j2"),
            Ok(vec![
                spec(Some("jump"), "j1", Some(2200)),
                spec(None, "j2", None)
            ])
        );
    }

    #[test]
    fn une_specification_ssh_url_avec_adresse_ipv6_est_lue() {
        assert_eq!(
            parse_list("ssh://user@[::1]:2222"),
            Ok(vec![spec(Some("user"), "::1", Some(2222))])
        );
        assert_eq!(
            parse_list("ssh://bastion.example"),
            Ok(vec![spec(None, "bastion.example", None)])
        );
    }

    #[test]
    fn une_adresse_ipv6_entre_crochets_sans_url_est_lue() {
        assert_eq!(
            parse_list("me@[fe80::1]:22"),
            Ok(vec![spec(Some("me"), "fe80::1", Some(22))])
        );
    }

    #[test]
    fn none_ou_une_valeur_vide_ne_donne_aucun_saut() {
        assert_eq!(parse_list("none"), Ok(Vec::new()));
        assert_eq!(parse_list(""), Ok(Vec::new()));
    }

    #[test]
    fn un_port_invalide_ou_un_hote_vide_ou_une_option_deguisee_est_refuse() {
        assert!(parse_list("host:0").is_err());
        assert!(parse_list("host:99999").is_err());
        assert!(parse_list("user@").is_err());
        assert!(parse_list("-oProxyCommand=evil").is_err());
        assert!(parse_list("good,-bad").is_err());
    }
}
