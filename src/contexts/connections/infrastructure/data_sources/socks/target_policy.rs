const MAX_NAME_LENGTH: usize = 253;
const MAX_LABEL_LENGTH: usize = 63;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPolicy {
    scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Scope {
    Subdomains(String),
    Exact(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AllowedTarget {
    host: String,
    port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TargetPolicyError {
    #[error("the MongoDB host is not a DNS host name")]
    InvalidHostName,
    #[error("the MongoDB SRV name needs at least three labels")]
    SrvNameTooShort,
}

impl TargetPolicy {
    pub fn from_srv_name(name: &str) -> Result<Self, TargetPolicyError> {
        let name = host_name(name).ok_or(TargetPolicyError::InvalidHostName)?;
        match parent_suffix(&name) {
            Some(suffix) => Ok(Self {
                scope: Scope::Subdomains(suffix),
            }),
            None => Err(TargetPolicyError::SrvNameTooShort),
        }
    }

    pub fn from_seed_host(host: &str) -> Result<Self, TargetPolicyError> {
        let host = host_name(host).ok_or(TargetPolicyError::InvalidHostName)?;
        let scope = match parent_suffix(&host) {
            Some(suffix) => Scope::Subdomains(suffix),
            None => Scope::Exact(host),
        };
        Ok(Self { scope })
    }

    pub fn admit(&self, host: &str, port: u16) -> Option<AllowedTarget> {
        if port == 0 {
            return None;
        }
        let host = host_name(host)?;
        let admitted = match &self.scope {
            Scope::Subdomains(suffix) => host.len() > suffix.len() && host.ends_with(suffix),
            Scope::Exact(name) => host == *name,
        };
        admitted.then_some(AllowedTarget { host, port })
    }
}

impl AllowedTarget {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

fn parent_suffix(name: &str) -> Option<String> {
    let (_, parent) = name.split_once('.')?;
    parent.contains('.').then(|| format!(".{parent}"))
}

fn host_name(raw: &str) -> Option<String> {
    if raw.len() > MAX_NAME_LENGTH {
        return None;
    }
    let name = raw.to_ascii_lowercase();
    let well_formed = name.split('.').all(is_label);
    let alphabetic_top = name
        .rsplit('.')
        .next()
        .is_some_and(|top| top.starts_with(|first: char| first.is_ascii_alphabetic()));
    (well_formed && alphabetic_top).then_some(name)
}

fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_LABEL_LENGTH
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRV_NAME: &str = "cluster0.atlas.scarif.test";
    const MEMBER: &str = "pl-0.atlas.scarif.test";

    fn srv_policy() -> TargetPolicy {
        TargetPolicy::from_srv_name(SRV_NAME).expect("valid SRV name")
    }

    fn seed_policy() -> TargetPolicy {
        TargetPolicy::from_seed_host(MEMBER).expect("valid seed host")
    }

    fn admitted(policy: &TargetPolicy, host: &str, port: u16) -> Option<(String, u16)> {
        policy
            .admit(host, port)
            .map(|target| (target.host().to_string(), target.port()))
    }

    #[test]
    fn un_membre_du_domaine_parent_du_nom_srv_est_admis() {
        for port in [1024, 1025, 1026] {
            assert_eq!(
                admitted(&srv_policy(), MEMBER, port),
                Some((MEMBER.to_string(), port))
            );
        }
        assert!(srv_policy().admit("pl-1.atlas.scarif.test", 1024).is_some());
        assert!(srv_policy().admit("a.b.atlas.scarif.test", 1024).is_some());
    }

    #[test]
    fn sans_srv_le_domaine_parent_de_l_hote_declare_est_admis() {
        assert_eq!(
            admitted(&seed_policy(), MEMBER, 1025),
            Some((MEMBER.to_string(), 1025))
        );
        assert!(
            seed_policy()
                .admit("pl-1.atlas.scarif.test", 1026)
                .is_some()
        );
    }

    #[test]
    fn un_hote_d_un_autre_domaine_est_refuse() {
        for policy in [srv_policy(), seed_policy()] {
            for host in ["evil.example.com", "scarif.test", "pl-0.other.scarif.test"] {
                assert_eq!(policy.admit(host, 1024), None, "{host}");
            }
        }
    }

    #[test]
    fn un_suffixe_ressemblant_est_refuse() {
        for policy in [srv_policy(), seed_policy()] {
            for host in [
                "evilatlas.scarif.test",
                "pl-0.evilatlas.scarif.test",
                "pl-0.atlas.scarif.test.example.com",
                "pl-0.atlas.scarif.testing",
                "pl-0-atlas.scarif.test",
            ] {
                assert_eq!(policy.admit(host, 1024), None, "{host}");
            }
        }
    }

    #[test]
    fn le_domaine_parent_lui_meme_est_refuse() {
        assert_eq!(srv_policy().admit("atlas.scarif.test", 1024), None);
        assert_eq!(seed_policy().admit("atlas.scarif.test", 1024), None);
    }

    #[test]
    fn une_adresse_ip_litterale_est_refusee() {
        for host in [
            "127.0.0.1",
            "10.1.0.226",
            "::1",
            "[::1]",
            "127.1",
            "0x7f.0.0.1",
            "2130706433",
            "1.2.3.atlas.scarif.test.4",
        ] {
            assert_eq!(srv_policy().admit(host, 22), None, "{host}");
        }
    }

    #[test]
    fn la_casse_est_normalisee_et_le_nom_transmis_est_celui_qui_a_ete_verifie() {
        assert_eq!(
            admitted(&srv_policy(), "PL-0.Atlas.Scarif.TEST", 1025),
            Some((MEMBER.to_string(), 1025))
        );
        let upper = TargetPolicy::from_srv_name("CLUSTER0.ATLAS.SCARIF.TEST").expect("valid");
        assert!(upper.admit(MEMBER, 1025).is_some());
    }

    #[test]
    fn un_point_final_ou_un_nom_mal_forme_est_refuse() {
        for host in [
            "pl-0.atlas.scarif.test.",
            "pl-0..atlas.scarif.test",
            ".atlas.scarif.test",
            "pl 0.atlas.scarif.test",
            "pl-0.atlas.scarif.test\0",
            "pl-0.atlas.scarif.test\0.evil.example.com",
            "pl-0.atlas.scarif.test:1024",
            "pl-0.atlas.scarif.test/x",
            "pl_0.atlas.scarif.test",
            "-pl.atlas.scarif.test",
            "pl-.atlas.scarif.test",
            "*.atlas.scarif.test",
            "pl-0.atl\u{0430}s.scarif.test",
            "user@pl-0.atlas.scarif.test",
            "",
        ] {
            assert_eq!(srv_policy().admit(host, 1024), None, "{host:?}");
        }
    }

    #[test]
    fn un_label_ou_un_nom_trop_long_est_refuse() {
        let long_label = format!("{}.atlas.scarif.test", "a".repeat(64));
        assert_eq!(srv_policy().admit(&long_label, 1024), None);
        let longest_label = format!("{}.atlas.scarif.test", "a".repeat(63));
        assert!(srv_policy().admit(&longest_label, 1024).is_some());
        let long_name = format!("{}atlas.scarif.test", "a.".repeat(120));
        assert!(long_name.len() > 253);
        assert_eq!(srv_policy().admit(&long_name, 1024), None);
    }

    #[test]
    fn le_port_zero_est_refuse() {
        assert_eq!(srv_policy().admit(MEMBER, 0), None);
    }

    #[test]
    fn un_nom_srv_de_moins_de_trois_labels_est_refuse() {
        assert_eq!(
            TargetPolicy::from_srv_name("scarif.test"),
            Err(TargetPolicyError::SrvNameTooShort)
        );
        assert_eq!(
            TargetPolicy::from_srv_name("test"),
            Err(TargetPolicyError::SrvNameTooShort)
        );
    }

    #[test]
    fn un_nom_srv_ou_un_hote_declare_invalide_est_refuse() {
        for name in [
            "10.0.0.1",
            "::1",
            "cluster0.atlas.scarif.test.",
            "",
            "a..b.c",
        ] {
            assert_eq!(
                TargetPolicy::from_srv_name(name),
                Err(TargetPolicyError::InvalidHostName),
                "{name:?}"
            );
            assert_eq!(
                TargetPolicy::from_seed_host(name),
                Err(TargetPolicyError::InvalidHostName),
                "{name:?}"
            );
        }
    }

    #[test]
    fn un_hote_declare_de_moins_de_trois_labels_n_admet_que_lui_meme() {
        let policy = TargetPolicy::from_seed_host("mongo.test").expect("valid seed host");
        assert_eq!(
            admitted(&policy, "mongo.test", 27017),
            Some(("mongo.test".to_string(), 27017))
        );
        for host in ["other.test", "a.mongo.test", "evilmongo.test"] {
            assert_eq!(policy.admit(host, 27017), None, "{host}");
        }
        let single = TargetPolicy::from_seed_host("mongodb").expect("valid seed host");
        assert!(single.admit("mongodb", 27017).is_some());
        assert_eq!(single.admit("a.mongodb", 27017), None);
    }
}
