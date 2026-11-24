use std::ffi::OsString;
use std::path::{Path, PathBuf};

const DIRECTORY: &str = "scarif";
const LEGACY_DIRECTORY: &str = "pg-pseudo";
const FILE_NAME: &str = "connections.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub explicit: bool,
    pub legacy: bool,
}

pub fn locate(
    configured: Option<&Path>,
    xdg_config_home: Option<OsString>,
    home: Option<&Path>,
) -> Option<Location> {
    if let Some(path) = configured {
        return Some(Location {
            path: path.to_path_buf(),
            explicit: true,
            legacy: false,
        });
    }
    let file = resolve_user_config_file(FILE_NAME, xdg_config_home, home)?;
    Some(Location {
        path: file.path,
        explicit: false,
        legacy: file.legacy,
    })
}

pub fn locate_for_write(
    configured: Option<&Path>,
    xdg_config_home: Option<OsString>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    match configured {
        Some(path) => Some(path.to_path_buf()),
        None => user_config_file(FILE_NAME, xdg_config_home, home),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserConfigFile {
    pub path: PathBuf,
    pub legacy: bool,
}

fn config_base(xdg_config_home: Option<OsString>, home: Option<&Path>) -> Option<PathBuf> {
    xdg_config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")))
}

pub fn user_config_file(
    file_name: &str,
    xdg_config_home: Option<OsString>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    Some(
        config_base(xdg_config_home, home)?
            .join(DIRECTORY)
            .join(file_name),
    )
}

pub fn resolve_user_config_file(
    file_name: &str,
    xdg_config_home: Option<OsString>,
    home: Option<&Path>,
) -> Option<UserConfigFile> {
    let base = config_base(xdg_config_home, home)?;
    let current = base.join(DIRECTORY).join(file_name);
    let legacy = base.join(LEGACY_DIRECTORY).join(file_name);
    if !current.exists() && legacy.exists() {
        return Some(UserConfigFile {
            path: legacy,
            legacy: true,
        });
    }
    Some(UserConfigFile {
        path: current,
        legacy: false,
    })
}

pub fn locate_from_environment(configured: Option<&Path>) -> Option<Location> {
    let home = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf());
    locate(
        configured,
        std::env::var_os("XDG_CONFIG_HOME"),
        home.as_deref(),
    )
}

#[cfg(test)]
mod unit {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/me")
    }

    #[test]
    fn la_variable_d_environnement_donne_le_chemin_explicite() {
        let location = locate(
            Some(Path::new("/shared/connections.toml")),
            Some(OsString::from("/xdg")),
            Some(&home()),
        )
        .unwrap();
        assert_eq!(
            location,
            Location {
                path: PathBuf::from("/shared/connections.toml"),
                explicit: true,
                legacy: false
            }
        );
    }

    #[test]
    fn xdg_config_home_absolu_est_utilise_sans_etre_explicite() {
        let location = locate(None, Some(OsString::from("/xdg")), Some(&home())).unwrap();
        assert_eq!(
            location,
            Location {
                path: PathBuf::from("/xdg/scarif/connections.toml"),
                explicit: false,
                legacy: false
            }
        );
    }

    #[test]
    fn xdg_config_home_relatif_ou_vide_est_ignore() {
        for value in ["relative/dir", ""] {
            let location = locate(None, Some(OsString::from(value)), Some(&home())).unwrap();
            assert_eq!(
                location.path,
                PathBuf::from("/home/me/.config/scarif/connections.toml")
            );
        }
    }

    #[test]
    fn par_defaut_le_fichier_est_dans_config_du_dossier_personnel() {
        let location = locate(None, None, Some(&home())).unwrap();
        assert_eq!(
            location.path,
            PathBuf::from("/home/me/.config/scarif/connections.toml")
        );
        assert!(!location.explicit);
    }

    #[test]
    fn un_autre_fichier_de_configuration_suit_les_memes_regles_d_emplacement() {
        assert_eq!(
            user_config_file("columns.toml", Some(OsString::from("/xdg")), Some(&home())),
            Some(PathBuf::from("/xdg/scarif/columns.toml"))
        );
        assert_eq!(
            user_config_file(
                "columns.toml",
                Some(OsString::from("relative")),
                Some(&home())
            ),
            Some(PathBuf::from("/home/me/.config/scarif/columns.toml"))
        );
        assert_eq!(user_config_file("columns.toml", None, None), None);
    }

    fn write_config(home: &Path, directory: &str, file: &str) -> PathBuf {
        let folder = home.join(".config").join(directory);
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(file);
        std::fs::write(&path, "").unwrap();
        path
    }

    #[test]
    fn le_fichier_du_nouveau_dossier_est_lu_meme_si_l_ancien_existe() {
        let dir = tempfile::tempdir().unwrap();
        let new = write_config(dir.path(), "scarif", "connections.toml");
        write_config(dir.path(), "pg-pseudo", "connections.toml");
        let location = locate(None, None, Some(dir.path())).unwrap();
        assert_eq!(location.path, new);
        assert!(!location.legacy);
    }

    #[test]
    fn le_fichier_de_l_ancien_dossier_est_lu_quand_le_nouveau_est_absent() {
        let dir = tempfile::tempdir().unwrap();
        let old = write_config(dir.path(), "pg-pseudo", "connections.toml");
        let location = locate(None, None, Some(dir.path())).unwrap();
        assert_eq!(location.path, old);
        assert!(location.legacy);
        assert!(!dir.path().join(".config/scarif").exists());
        let resolved =
            resolve_user_config_file("connections.toml", None, Some(dir.path())).unwrap();
        assert_eq!(
            resolved,
            UserConfigFile {
                path: old,
                legacy: true
            }
        );
    }

    #[test]
    fn la_reecriture_vise_toujours_le_nouveau_dossier() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "pg-pseudo", "connections.toml");
        assert_eq!(
            locate_for_write(None, None, Some(dir.path())),
            Some(dir.path().join(".config/scarif/connections.toml"))
        );
    }

    #[test]
    fn un_chemin_explicite_ignore_l_ancien_dossier() {
        let dir = tempfile::tempdir().unwrap();
        write_config(dir.path(), "pg-pseudo", "connections.toml");
        let location = locate(Some(Path::new("/shared/c.toml")), None, Some(dir.path())).unwrap();
        assert!(location.explicit && !location.legacy);
    }

    #[test]
    fn sans_dossier_personnel_ni_xdg_aucun_emplacement_n_est_connu() {
        assert_eq!(locate(None, None, None), None);
    }
}
