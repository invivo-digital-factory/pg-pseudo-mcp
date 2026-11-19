use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use super::credentials::Secret;

pub struct Target<'a> {
    pub host: &'a str,
    pub port: u16,
    pub database: &'a str,
    pub user: &'a str,
}

pub fn default_path() -> Option<PathBuf> {
    locate(
        std::env::var_os("PGPASSFILE"),
        directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()),
        std::env::var_os("APPDATA"),
    )
}

fn locate(
    pgpassfile: Option<OsString>,
    home: Option<PathBuf>,
    appdata: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(path) = pgpassfile.filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(path));
    }
    if cfg!(windows) {
        appdata
            .filter(|dir| !dir.is_empty())
            .map(|dir| PathBuf::from(dir).join("postgresql").join("pgpass.conf"))
    } else {
        home.map(|home| home.join(".pgpass"))
    }
}

pub fn lookup(path: &Path, target: &Target<'_>) -> Option<Secret> {
    let bytes = fs::read(path).ok()?;
    let contents = String::from_utf8_lossy(&bytes);
    let port = target.port.to_string();
    let wanted = [target.host, port.as_str(), target.database, target.user];
    contents.lines().find_map(|line| {
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (fields, password) = split_line(line)?;
        fields
            .iter()
            .zip(wanted)
            .all(|(field, value)| field.matches(value))
            .then(|| Secret::new(password))
    })
}

struct Field {
    value: String,
    wildcard: bool,
}

impl Field {
    fn matches(&self, candidate: &str) -> bool {
        self.wildcard || self.value == candidate
    }
}

fn split_line(line: &str) -> Option<([Field; 4], String)> {
    let mut chars = line.chars();
    let mut next_field = || {
        let mut value = String::new();
        let mut raw = String::new();
        loop {
            match chars.next()? {
                ':' => break,
                '\\' => {
                    let escaped = chars.next().unwrap_or('\\');
                    raw.push('\\');
                    raw.push(escaped);
                    value.push(escaped);
                }
                other => {
                    raw.push(other);
                    value.push(other);
                }
            }
        }
        Some(Field {
            wildcard: raw == "*",
            value,
        })
    };
    let fields = [next_field()?, next_field()?, next_field()?, next_field()?];
    Some((fields, unescape(chars.as_str())))
}

fn unescape(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(next) = chars.next() {
        if next == '\\' {
            output.push(chars.next().unwrap_or('\\'));
        } else {
            output.push(next);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target<'static> {
        Target {
            host: "db.internal.example",
            port: 5433,
            database: "shop",
            user: "reader",
        }
    }

    fn write(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("pgpass");
        fs::write(&path, contents).unwrap();
        (dir, path)
    }

    fn found(contents: &str) -> Option<String> {
        let (_dir, path) = write(contents);
        lookup(&path, &target()).map(|secret| secret.expose().to_string())
    }

    #[test]
    fn exact_line_matches() {
        assert_eq!(
            found("db.internal.example:5433:shop:reader:pw\n").as_deref(),
            Some("pw")
        );
    }

    #[test]
    fn wildcards_match_any_value() {
        assert_eq!(found("*:*:*:reader:pw").as_deref(), Some("pw"));
        assert_eq!(found("*:5433:*:*:pw2").as_deref(), Some("pw2"));
    }

    #[test]
    fn first_matching_line_wins() {
        let contents = "other:5433:shop:reader:no\n*:*:shop:reader:first\n*:*:*:*:second\n";
        assert_eq!(found(contents).as_deref(), Some("first"));
    }

    #[test]
    fn comments_blank_and_short_lines_are_skipped() {
        let contents = "# *:*:*:*:commented\n\n*:*:shop\n*:*:*:*:real\r\n";
        assert_eq!(found(contents).as_deref(), Some("real"));
    }

    #[test]
    fn escaped_colons_and_backslashes_are_unescaped() {
        assert_eq!(
            found(r"*:*:*:reader:p\:a\\ss:word").as_deref(),
            Some(r"p:a\ss:word")
        );
        let target = Target {
            host: "h:1",
            port: 5432,
            database: r"d\b",
            user: "u",
        };
        let (_dir, path) = write(r"h\:1:5432:d\\b:u:pw");
        assert_eq!(lookup(&path, &target).unwrap().expose(), "pw");
    }

    #[test]
    fn escaped_star_is_literal() {
        assert_eq!(found(r"\*:*:*:*:pw"), None);
    }

    #[test]
    fn no_match_and_missing_file_return_none() {
        assert_eq!(found("db.internal.example:5432:shop:reader:pw"), None);
        assert_eq!(found("db.internal.example:5433:shop:writer:pw"), None);
        let dir = tempfile::TempDir::new().unwrap();
        assert!(lookup(&dir.path().join("absent"), &target()).is_none());
    }

    #[test]
    fn pgpassfile_wins_over_the_home_default() {
        let home = PathBuf::from("/home/someone");
        assert_eq!(
            locate(
                Some(OsString::from("/custom/pgpass")),
                Some(home.clone()),
                None
            ),
            Some(PathBuf::from("/custom/pgpass"))
        );
        assert_eq!(
            locate(Some(OsString::new()), Some(home.clone()), None),
            Some(default_in(&home, None))
        );
        assert_eq!(
            locate(None, Some(home.clone()), None),
            Some(default_in(&home, None))
        );
    }

    fn default_in(home: &Path, appdata: Option<&Path>) -> PathBuf {
        if cfg!(windows) {
            appdata.unwrap().join("postgresql").join("pgpass.conf")
        } else {
            home.join(".pgpass")
        }
    }

    #[test]
    fn secret_is_never_printed() {
        let (_dir, path) = write("*:*:*:*:hunter2-canary");
        let secret = lookup(&path, &target()).unwrap();
        assert!(!format!("{secret:?}").contains("hunter2-canary"));
    }
}
