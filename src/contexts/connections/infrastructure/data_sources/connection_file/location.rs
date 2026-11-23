use std::ffi::OsString;
use std::path::{Path, PathBuf};

const DIRECTORY: &str = "pg-pseudo";
const FILE_NAME: &str = "connections.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub explicit: bool,
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
        });
    }
    Some(Location {
        path: user_config_file(FILE_NAME, xdg_config_home, home)?,
        explicit: false,
    })
}

pub fn user_config_file(
    file_name: &str,
    xdg_config_home: Option<OsString>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    let base = xdg_config_home
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")))?;
    Some(base.join(DIRECTORY).join(file_name))
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
                explicit: true
            }
        );
    }

    #[test]
    fn xdg_config_home_absolu_est_utilise_sans_etre_explicite() {
        let location = locate(None, Some(OsString::from("/xdg")), Some(&home())).unwrap();
        assert_eq!(
            location,
            Location {
                path: PathBuf::from("/xdg/pg-pseudo/connections.toml"),
                explicit: false
            }
        );
    }

    #[test]
    fn xdg_config_home_relatif_ou_vide_est_ignore() {
        for value in ["relative/dir", ""] {
            let location = locate(None, Some(OsString::from(value)), Some(&home())).unwrap();
            assert_eq!(
                location.path,
                PathBuf::from("/home/me/.config/pg-pseudo/connections.toml")
            );
        }
    }

    #[test]
    fn par_defaut_le_fichier_est_dans_config_du_dossier_personnel() {
        let location = locate(None, None, Some(&home())).unwrap();
        assert_eq!(
            location.path,
            PathBuf::from("/home/me/.config/pg-pseudo/connections.toml")
        );
        assert!(!location.explicit);
    }

    #[test]
    fn un_autre_fichier_de_configuration_suit_les_memes_regles_d_emplacement() {
        assert_eq!(
            user_config_file("columns.toml", Some(OsString::from("/xdg")), Some(&home())),
            Some(PathBuf::from("/xdg/pg-pseudo/columns.toml"))
        );
        assert_eq!(
            user_config_file(
                "columns.toml",
                Some(OsString::from("relative")),
                Some(&home())
            ),
            Some(PathBuf::from("/home/me/.config/pg-pseudo/columns.toml"))
        );
        assert_eq!(user_config_file("columns.toml", None, None), None);
    }

    #[test]
    fn sans_dossier_personnel_ni_xdg_aucun_emplacement_n_est_connu() {
        assert_eq!(locate(None, None, None), None);
    }
}
