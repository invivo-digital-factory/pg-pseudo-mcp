use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::{Algorithm, HashAlg, PublicKey};
use sha1::Sha1;

pub(super) fn known_hosts_path(
    configured: Option<&Path>,
    home: Option<PathBuf>,
) -> Result<PathBuf, String> {
    match (configured, home) {
        (Some(path), _) => Ok(path.to_path_buf()),
        (None, Some(home)) => Ok(home.join(".ssh").join("known_hosts")),
        (None, None) => Err(
            "cannot locate the SSH known_hosts file: no home directory found; set PG_PSEUDO_SSH_KNOWN_HOSTS to an absolute path".to_string(),
        ),
    }
}

pub(super) fn home_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())
}

pub(super) fn recorded_algorithms(host: &str, port: u16, known_hosts: &Path) -> Vec<Algorithm> {
    known_keys(host, port, known_hosts)
        .map(|keys| {
            keys.into_iter()
                .filter(|known| !known.revoked)
                .map(|known| known.key.algorithm())
                .collect()
        })
        .unwrap_or_default()
}

struct KnownKey {
    line: usize,
    revoked: bool,
    key: PublicKey,
}

fn known_keys(host: &str, port: u16, known_hosts: &Path) -> Result<Vec<KnownKey>, String> {
    let contents = match std::fs::read(known_hosts) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let name = if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    };
    Ok(contents
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            parse_entry(line, &name).map(|(revoked, key)| KnownKey {
                line: index + 1,
                revoked,
                key,
            })
        })
        .collect())
}

fn parse_entry(line: &str, name: &str) -> Option<(bool, PublicKey)> {
    let mut fields = line.split_whitespace();
    let first = fields.next().filter(|field| !field.starts_with('#'))?;
    let (revoked, patterns) = match first {
        "@revoked" => (true, fields.next()?),
        marker if marker.starts_with('@') => return None,
        patterns => (false, patterns),
    };
    if !patterns
        .split(',')
        .any(|pattern| host_matches(pattern, name))
    {
        return None;
    }
    let algorithm = fields.next()?;
    let data = fields.next()?;
    let key = PublicKey::from_openssh(&format!("{algorithm} {data}")).ok()?;
    Some((revoked, key))
}

fn host_matches(pattern: &str, name: &str) -> bool {
    let Some(hashed) = pattern.strip_prefix("|1|") else {
        return pattern == name;
    };
    let Some((salt, hash)) = hashed.split_once('|') else {
        return false;
    };
    let engine = base64::engine::general_purpose::STANDARD;
    let (Ok(salt), Ok(hash)) = (engine.decode(salt), engine.decode(hash)) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(&salt) else {
        return false;
    };
    mac.update(name.as_bytes());
    mac.verify_slice(&hash).is_ok()
}

pub(super) fn known_algorithms_first(
    preferred: &[Algorithm],
    recorded: &[Algorithm],
) -> Vec<Algorithm> {
    let is_recorded = |algorithm: &Algorithm| {
        recorded.iter().any(|known| {
            known == algorithm
                || matches!(
                    (known, algorithm),
                    (Algorithm::Rsa { .. }, Algorithm::Rsa { .. })
                )
        })
    };
    let (known, unknown): (Vec<Algorithm>, Vec<Algorithm>) =
        preferred.iter().cloned().partition(is_recorded);
    known.into_iter().chain(unknown).collect()
}

pub(super) fn verify_host_key(
    host: &str,
    port: u16,
    key: &PublicKey,
    known_hosts: &Path,
) -> Result<(), String> {
    let fingerprint = Fingerprint(key);
    let file = known_hosts.display();
    let recorded = known_keys(host, port, known_hosts).map_err(|error| {
        format!("cannot check the SSH host key of {host}:{port} against {file}: {error}")
    })?;
    let same_key = |known: &&KnownKey| known.key.key_data() == key.key_data();
    if let Some(known) = recorded.iter().filter(|known| known.revoked).find(same_key) {
        return Err(format!(
            "SSH host key of {host}:{port} ({fingerprint}) is revoked by line {} of {file}; refusing to connect",
            known.line
        ));
    }
    let trusted: Vec<&KnownKey> = recorded.iter().filter(|known| !known.revoked).collect();
    if trusted.iter().any(same_key) {
        return Ok(());
    }
    if let Some(known) = trusted
        .iter()
        .find(|known| known.key.algorithm() == key.algorithm())
    {
        return Err(format!(
            "SSH host key of {host}:{port} changed: the server now presents {fingerprint}, which differs from line {} of {file}; refusing to connect until the new key is confirmed with the server administrator and the old entry is replaced",
            known.line
        ));
    }
    if let Some(known) = trusted.first() {
        return Err(format!(
            "SSH server {host}:{port} presented a host key ({fingerprint}) whose type is not recorded in {file}, but a key of another type ({}) is already known for this host on line {}; the connection is refused because this may be an interception; confirm the server keys with the server administrator",
            known.key.algorithm(),
            known.line
        ));
    }
    Err(format!(
        "SSH host key of {host}:{port} ({fingerprint}) is not in known_hosts file {file}; check this fingerprint with the server administrator, then add it with `ssh-keyscan -p {port} {host} >> {file}`"
    ))
}

struct Fingerprint<'a>(&'a PublicKey);

impl fmt::Display for Fingerprint<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}",
            self.0.algorithm(),
            self.0.fingerprint(HashAlg::Sha256)
        )
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use hmac::{Hmac, KeyInit, Mac};
    use russh::client;
    use russh::keys::EcdsaCurve;
    use sha1::Sha1;

    use super::*;

    const HOST: &str = "bastion.example";
    const PORT: u16 = 2222;

    fn key_line(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("ssh")
            .join(name);
        let line = std::fs::read_to_string(path).unwrap();
        let fields: Vec<&str> = line.split_whitespace().take(2).collect();
        fields.join(" ")
    }

    fn key(name: &str) -> PublicKey {
        PublicKey::from_openssh(&key_line(name)).unwrap()
    }

    fn server() -> PublicKey {
        key("ssh_host_ed25519_test.pub")
    }

    fn check(contents: &str) -> Result<(), String> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        std::fs::write(&path, contents).unwrap();
        verify_host_key(HOST, PORT, &server(), &path)
    }

    fn pattern() -> String {
        format!("[{HOST}]:{PORT}")
    }

    fn hashed(name: &str) -> String {
        let salt = [7u8; 20];
        let mut mac = Hmac::<Sha1>::new_from_slice(&salt).unwrap();
        mac.update(name.as_bytes());
        let engine = base64::engine::general_purpose::STANDARD;
        format!(
            "|1|{}|{}",
            engine.encode(salt),
            engine.encode(mac.finalize().into_bytes())
        )
    }

    #[test]
    fn a_matching_line_is_accepted_even_after_another_key_of_the_same_type() {
        let contents = format!(
            "{p} {other}\n{p} {own}\n",
            p = pattern(),
            other = key_line("other_host_ed25519_test.pub"),
            own = key_line("ssh_host_ed25519_test.pub"),
        );
        assert_eq!(check(&contents), Ok(()));
    }

    #[test]
    fn a_revoked_key_is_refused_even_when_also_listed() {
        let own = key_line("ssh_host_ed25519_test.pub");
        let contents = format!("{p} {own}\n@revoked {p} {own}\n", p = pattern());
        let message = check(&contents).unwrap_err();
        assert!(message.contains("revoked"), "{message}");
        assert!(message.contains("SHA256:"), "{message}");
        let contents = format!("@revoked * {own}\n{p} {own}\n", p = pattern());
        assert_eq!(check(&contents), Ok(()));
    }

    #[test]
    fn a_different_key_of_the_same_type_is_reported_as_changed() {
        let contents = format!(
            "# comment\n{} {}\n",
            pattern(),
            key_line("other_host_ed25519_test.pub")
        );
        let message = check(&contents).unwrap_err();
        assert!(message.contains("changed"), "{message}");
        assert!(message.contains("line 2"), "{message}");
        assert!(!message.contains("ssh-keyscan"), "{message}");
    }

    #[test]
    fn a_key_of_another_type_already_known_refuses_without_suggesting_keyscan() {
        let contents = format!("{} {}\n", pattern(), key_line("ssh_host_ecdsa_test.pub"));
        let message = check(&contents).unwrap_err();
        assert!(message.contains("another type"), "{message}");
        assert!(message.contains("interception"), "{message}");
        assert!(message.contains("refus"), "{message}");
        assert!(!message.contains("ssh-keyscan"), "{message}");
        assert!(!message.contains("changed"), "{message}");
    }

    #[test]
    fn an_unknown_host_suggests_keyscan() {
        let contents = format!("other.example {}\n", key_line("ssh_host_ed25519_test.pub"));
        let message = check(&contents).unwrap_err();
        assert!(message.contains("not in known_hosts"), "{message}");
        assert!(
            message.contains("ssh-keyscan -p 2222 bastion.example"),
            "{message}"
        );
    }

    #[test]
    fn hashed_and_bracketed_port_entries_are_matched() {
        let own = key_line("ssh_host_ed25519_test.pub");
        assert_eq!(check(&format!("{} {own}\n", hashed(&pattern()))), Ok(()));
        assert_eq!(
            check(&format!("other.example,{}\t{own} comment\n", pattern())),
            Ok(())
        );
        let message = check(&format!("{HOST} {own}\n")).unwrap_err();
        assert!(message.contains("not in known_hosts"), "{message}");
        let message = check(&format!("{} {own}\n", hashed(HOST))).unwrap_err();
        assert!(message.contains("not in known_hosts"), "{message}");
    }

    #[test]
    fn cert_authority_lines_are_ignored() {
        let contents = format!(
            "@cert-authority {} {}\n",
            pattern(),
            key_line("ssh_host_ed25519_test.pub")
        );
        let message = check(&contents).unwrap_err();
        assert!(message.contains("not in known_hosts"), "{message}");
    }

    #[test]
    fn a_missing_known_hosts_file_reports_an_unknown_host() {
        let dir = tempfile::tempdir().unwrap();
        let message =
            verify_host_key(HOST, PORT, &server(), &dir.path().join("absent")).unwrap_err();
        assert!(message.contains("not in known_hosts"), "{message}");
    }

    #[test]
    fn revoked_keys_are_not_offered_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("known_hosts");
        std::fs::write(
            &path,
            format!(
                "@revoked {p} {ecdsa}\n{p} {ed}\n",
                p = pattern(),
                ecdsa = key_line("ssh_host_ecdsa_test.pub"),
                ed = key_line("ssh_host_ed25519_test.pub"),
            ),
        )
        .unwrap();
        assert_eq!(
            recorded_algorithms(HOST, PORT, &path),
            vec![Algorithm::Ed25519]
        );
    }

    #[test]
    fn the_default_known_hosts_needs_a_home_directory() {
        let configured = PathBuf::from("/etc/ssh/known_hosts_custom");
        assert_eq!(
            known_hosts_path(Some(&configured), None).unwrap(),
            configured
        );
        assert_eq!(
            known_hosts_path(None, Some(PathBuf::from("/home/someone"))).unwrap(),
            PathBuf::from("/home/someone/.ssh/known_hosts")
        );
        let message = known_hosts_path(None, None).unwrap_err();
        assert!(message.contains("home directory"), "{message}");
    }

    #[test]
    fn host_key_types_recorded_in_known_hosts_are_offered_first() {
        let preferred = client::Config::default().preferred.key.into_owned();
        let p256 = Algorithm::Ecdsa {
            curve: EcdsaCurve::NistP256,
        };
        let ordered = known_algorithms_first(&preferred, std::slice::from_ref(&p256));
        assert_eq!(ordered.first(), Some(&p256));
        assert_eq!(ordered.len(), preferred.len());
        let ordered = known_algorithms_first(&preferred, &[Algorithm::Rsa { hash: None }]);
        assert!(
            ordered
                .iter()
                .take(3)
                .all(|algorithm| matches!(algorithm, Algorithm::Rsa { .. })),
            "{ordered:?}"
        );
        assert_eq!(known_algorithms_first(&preferred, &[]), preferred);
    }
}
