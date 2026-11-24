use crate::contexts::connections::domain::types::DatabaseNames;

const SYSTEM_DATABASES: [&str; 3] = ["admin", "local", "config"];
const SYSTEM_COLLECTION_PREFIX: &str = "system.";

pub fn visible_databases(names: DatabaseNames) -> Vec<String> {
    let mut visible: Vec<String> = names
        .authorized
        .into_iter()
        .chain(names.default)
        .filter(|name| !SYSTEM_DATABASES.contains(&name.as_str()))
        .collect();
    visible.sort();
    visible.dedup();
    visible
}

pub fn is_listed_collection(name: &str) -> bool {
    !name.starts_with(SYSTEM_COLLECTION_PREFIX)
}

#[cfg(test)]
mod unit {
    use super::*;

    fn names(authorized: &[&str], default: Option<&str>) -> DatabaseNames {
        DatabaseNames {
            authorized: authorized.iter().map(ToString::to_string).collect(),
            default: default.map(str::to_string),
        }
    }

    #[test]
    fn les_bases_systeme_sont_retirees_des_bases_autorisees() {
        assert_eq!(
            visible_databases(names(
                &["admin", "shop", "local", "config", "billing"],
                None
            )),
            vec!["billing".to_string(), "shop".to_string()]
        );
    }

    #[test]
    fn la_base_par_defaut_declaree_est_ajoutee_une_seule_fois() {
        assert_eq!(
            visible_databases(names(&["shop"], Some("archive"))),
            vec!["archive".to_string(), "shop".to_string()]
        );
        assert_eq!(
            visible_databases(names(&["shop"], Some("shop"))),
            vec!["shop".to_string()]
        );
    }

    #[test]
    fn une_base_par_defaut_systeme_n_est_pas_ajoutee() {
        assert!(visible_databases(names(&[], Some("admin"))).is_empty());
    }

    #[test]
    fn les_collections_systeme_ne_sont_pas_listees() {
        assert!(is_listed_collection("customers"));
        assert!(is_listed_collection("fs.files"));
        assert!(is_listed_collection("systems"));
        assert!(!is_listed_collection("system.views"));
        assert!(!is_listed_collection("system.buckets.metrics"));
    }
}
