use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PathError {
    Token(char),
    NoHome,
    NoLocalUser,
}

pub(super) struct PathContext<'a> {
    pub home: Option<&'a Path>,
    pub local_user: Option<&'a str>,
    pub hostname: &'a str,
    pub port: u16,
    pub remote_user: &'a str,
}

const REFUSED: [(&str, &str); 8] = [
    ("proxycommand", "ProxyCommand"),
    ("proxyusefdpass", "ProxyUseFdpass"),
    ("knownhostscommand", "KnownHostsCommand"),
    ("certificatefile", "CertificateFile"),
    ("pkcs11provider", "PKCS11Provider"),
    ("canonicalizehostname", "CanonicalizeHostname"),
    ("bindaddress", "BindAddress"),
    ("bindinterface", "BindInterface"),
];

pub(super) fn unsupported_directive(directives: &[(String, String)]) -> Option<&'static str> {
    REFUSED.iter().find_map(|(key, name)| {
        directives
            .iter()
            .any(|(directive, value)| directive == key && !is_default(value))
            .then_some(*name)
    })
}

fn is_default(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "none" | "no" | "false"
    )
}

pub(super) fn expand_path(template: &str, context: &PathContext<'_>) -> Result<PathBuf, PathError> {
    let home = || context.home.ok_or(PathError::NoHome);
    let template = match template {
        "~" => return Ok(home()?.to_path_buf()),
        other => match other.strip_prefix("~/") {
            Some(rest) => format!("{}/{rest}", home()?.display()),
            None => other.to_string(),
        },
    };
    let mut expanded = String::new();
    let mut characters = template.chars();
    while let Some(character) = characters.next() {
        if character != '%' {
            expanded.push(character);
            continue;
        }
        match characters.next() {
            Some('%') => expanded.push('%'),
            Some('d') => expanded.push_str(&home()?.to_string_lossy()),
            Some('h') => expanded.push_str(context.hostname),
            Some('p') => expanded.push_str(&context.port.to_string()),
            Some('r') => expanded.push_str(context.remote_user),
            Some('u') => expanded.push_str(context.local_user.ok_or(PathError::NoLocalUser)?),
            Some(other) => return Err(PathError::Token(other)),
            None => return Err(PathError::Token('%')),
        }
    }
    Ok(PathBuf::from(expanded))
}

#[cfg(test)]
mod unit {
    use super::*;

    fn directives(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    fn context() -> PathContext<'static> {
        PathContext {
            home: Some(Path::new("/home/me")),
            local_user: Some("me"),
            hostname: "db-bastion",
            port: 2222,
            remote_user: "alice",
        }
    }

    #[test]
    fn proxycommand_non_par_defaut_rend_la_connexion_non_prise_en_charge() {
        assert_eq!(
            unsupported_directive(&directives(&[("proxycommand", "nc %h %p")])),
            Some("ProxyCommand")
        );
        assert_eq!(
            unsupported_directive(&directives(&[("proxycommand", "none")])),
            None
        );
    }

    #[test]
    fn chaque_directive_refusee_est_nommee_avec_son_orthographe_openssh() {
        for (key, value, name) in [
            ("proxyusefdpass", "yes", "ProxyUseFdpass"),
            ("knownhostscommand", "/bin/lookup %H", "KnownHostsCommand"),
            ("certificatefile", "~/.ssh/id-cert.pub", "CertificateFile"),
            ("pkcs11provider", "/lib/p11.so", "PKCS11Provider"),
            ("canonicalizehostname", "yes", "CanonicalizeHostname"),
            ("bindaddress", "10.0.0.5", "BindAddress"),
            ("bindinterface", "en0", "BindInterface"),
        ] {
            assert_eq!(
                unsupported_directive(&directives(&[(key, value)])),
                Some(name),
                "{key}"
            );
        }
    }

    #[test]
    fn les_valeurs_par_defaut_et_les_directives_ignorees_ne_bloquent_pas() {
        let accepted = directives(&[
            ("proxyusefdpass", "no"),
            ("pkcs11provider", "none"),
            ("canonicalizehostname", "false"),
            ("stricthostkeychecking", "false"),
            ("updatehostkeys", "yes"),
            ("checkhostip", "no"),
            ("ciphers", "aes256-ctr"),
            ("controlmaster", "auto"),
            ("serveraliveinterval", "30"),
        ]);
        assert_eq!(unsupported_directive(&accepted), None);
    }

    #[test]
    fn les_jetons_et_le_tilde_d_un_chemin_sont_developpes() {
        assert_eq!(
            expand_path("%d/.ssh/k_%h", &context()),
            Ok(PathBuf::from("/home/me/.ssh/k_db-bastion"))
        );
        assert_eq!(
            expand_path("~/.ssh/id_%r_%p_%u_100%%", &context()),
            Ok(PathBuf::from("/home/me/.ssh/id_alice_2222_me_100%"))
        );
        assert_eq!(
            expand_path("/abs/key", &context()),
            Ok(PathBuf::from("/abs/key"))
        );
    }

    #[test]
    fn un_jeton_inconnu_est_refuse_avec_son_nom() {
        assert_eq!(expand_path("%C", &context()), Err(PathError::Token('C')));
        assert_eq!(
            expand_path("/keys/%", &context()),
            Err(PathError::Token('%'))
        );
    }

    #[test]
    fn un_tilde_sans_dossier_personnel_est_refuse() {
        let mut no_home = context();
        no_home.home = None;
        assert_eq!(expand_path("~/k", &no_home), Err(PathError::NoHome));
        assert_eq!(expand_path("%d/k", &no_home), Err(PathError::NoHome));
        let mut no_user = context();
        no_user.local_user = None;
        assert_eq!(expand_path("/k_%u", &no_user), Err(PathError::NoLocalUser));
    }
}
