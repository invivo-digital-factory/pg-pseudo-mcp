use std::collections::HashSet;

use crate::contexts::connections::domain::types::Discovery;

pub fn layer_sources(file: Discovery, dbeaver: Option<Discovery>, empty_hint: &str) -> Discovery {
    let file_names: HashSet<String> = file
        .connections
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    let (dbeaver_entries, dbeaver_hint) = match dbeaver {
        Some(discovery) => (discovery.connections, discovery.hint),
        None => (Vec::new(), None),
    };
    let mut connections = file.connections;
    connections.extend(dbeaver_entries.into_iter().filter(|entry| {
        !file_names.contains(&entry.name) && !file_names.contains(&entry.declared_name)
    }));
    connections.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    let mut hints: Vec<&str> = file.hint.as_deref().into_iter().collect();
    if connections.is_empty() {
        hints.extend(dbeaver_hint.as_deref());
        if file.hint.is_none() {
            hints.push(empty_hint);
        }
    }
    Discovery {
        connections,
        hint: (!hints.is_empty()).then(|| hints.join(" ")),
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use crate::contexts::connections::domain::services::disambiguate;
    use crate::contexts::connections::domain::types::{
        ConnectionEntry, ConnectionSource, ConnectionStatus,
    };

    fn entry(
        source: ConnectionSource,
        id: &str,
        name: &str,
        status: ConnectionStatus,
    ) -> ConnectionEntry {
        ConnectionEntry {
            id: id.to_string(),
            name: name.to_string(),
            declared_name: name.to_string(),
            source,
            folder: None,
            project: "General".to_string(),
            engine: "postgresql".to_string(),
            connection_type: "dev".to_string(),
            database: None,
            status,
            params: None,
        }
    }

    fn file(name: &str) -> ConnectionEntry {
        entry(
            ConnectionSource::File,
            name,
            name,
            ConnectionStatus::Available,
        )
    }

    fn dbeaver(id: &str, name: &str) -> ConnectionEntry {
        entry(
            ConnectionSource::Dbeaver,
            id,
            name,
            ConnectionStatus::Available,
        )
    }

    fn discovery(connections: Vec<ConnectionEntry>, hint: Option<&str>) -> Discovery {
        Discovery {
            connections,
            hint: hint.map(str::to_string),
        }
    }

    fn names(discovery: &Discovery) -> Vec<(String, ConnectionSource)> {
        discovery
            .connections
            .iter()
            .map(|entry| (entry.name.clone(), entry.source))
            .collect()
    }

    #[test]
    fn une_connexion_du_fichier_masque_celle_de_dbeaver_du_meme_nom() {
        let layered = layer_sources(
            discovery(vec![file("app-prod")], None),
            Some(discovery(
                vec![dbeaver("a", "app-prod"), dbeaver("b", "billing")],
                None,
            )),
            "empty",
        );
        assert_eq!(
            names(&layered),
            vec![
                ("app-prod".to_string(), ConnectionSource::File),
                ("billing".to_string(), ConnectionSource::Dbeaver)
            ]
        );
    }

    #[test]
    fn une_entree_du_fichier_indisponible_masque_quand_meme_l_entree_dbeaver() {
        let broken = entry(
            ConnectionSource::File,
            "app",
            "app",
            ConnectionStatus::Unavailable {
                reason: "unknown key".to_string(),
            },
        );
        let layered = layer_sources(
            discovery(vec![broken], None),
            Some(discovery(vec![dbeaver("a", "app")], None)),
            "empty",
        );
        assert_eq!(
            names(&layered),
            vec![("app".to_string(), ConnectionSource::File)]
        );
    }

    fn duplicated_dbeaver() -> Vec<ConnectionEntry> {
        let mut entries = vec![dbeaver("first", "app"), dbeaver("second", "app")];
        disambiguate(&mut entries);
        entries
    }

    #[test]
    fn un_nom_du_fichier_egal_au_nom_ecrit_dans_dbeaver_masque_les_deux_doublons() {
        let layered = layer_sources(
            discovery(vec![file("app")], None),
            Some(discovery(duplicated_dbeaver(), None)),
            "empty",
        );
        assert_eq!(
            names(&layered),
            vec![("app".to_string(), ConnectionSource::File)]
        );
    }

    #[test]
    fn les_deux_noms_suffixes_dans_le_fichier_masquent_les_deux_doublons() {
        let suffixed: Vec<String> = duplicated_dbeaver()
            .iter()
            .map(|entry| entry.name.clone())
            .collect();
        let layered = layer_sources(
            discovery(suffixed.iter().map(|name| file(name)).collect(), None),
            Some(discovery(duplicated_dbeaver(), None)),
            "empty",
        );
        assert_eq!(layered.connections.len(), 2);
        assert!(
            layered
                .connections
                .iter()
                .all(|entry| entry.source == ConnectionSource::File)
        );
    }

    #[test]
    fn un_seul_nom_suffixe_dans_le_fichier_ne_masque_que_ce_doublon() {
        let duplicates = duplicated_dbeaver();
        let hidden = duplicates[0].name.clone();
        let kept = duplicates[1].name.clone();
        let layered = layer_sources(
            discovery(vec![file(&hidden)], None),
            Some(discovery(duplicates, None)),
            "empty",
        );
        assert_eq!(names(&layered), {
            let mut expected = vec![
                (hidden, ConnectionSource::File),
                (kept, ConnectionSource::Dbeaver),
            ];
            expected.sort();
            expected
        });
    }

    #[test]
    fn sans_source_dbeaver_seules_les_connexions_du_fichier_restent() {
        let layered = layer_sources(discovery(vec![file("b"), file("a")], None), None, "empty");
        assert_eq!(
            names(&layered),
            vec![
                ("a".to_string(), ConnectionSource::File),
                ("b".to_string(), ConnectionSource::File)
            ]
        );
        assert_eq!(layered.hint, None);
    }

    #[test]
    fn l_indice_du_fichier_est_conserve_et_celui_de_dbeaver_n_apparait_que_sans_connexion() {
        let with_connections = layer_sources(
            discovery(vec![file("a")], Some("file hint")),
            Some(discovery(Vec::new(), Some("no DBeaver workspace"))),
            "create the file",
        );
        assert_eq!(with_connections.hint.as_deref(), Some("file hint"));
        let without = layer_sources(
            discovery(Vec::new(), Some("file hint")),
            Some(discovery(Vec::new(), Some("no DBeaver workspace"))),
            "create the file",
        );
        assert_eq!(
            without.hint.as_deref(),
            Some("file hint no DBeaver workspace")
        );
    }

    #[test]
    fn sans_aucune_connexion_l_indice_propose_de_creer_le_fichier_ou_d_importer() {
        let empty = layer_sources(
            discovery(Vec::new(), None),
            Some(discovery(Vec::new(), None)),
            "create the file",
        );
        assert_eq!(empty.hint.as_deref(), Some("create the file"));
        let dbeaver_off = layer_sources(discovery(Vec::new(), None), None, "create the file");
        assert_eq!(dbeaver_off.hint.as_deref(), Some("create the file"));
    }
    #[test]
    fn un_fichier_vide_qui_porte_deja_son_indice_n_y_ajoute_pas_l_indice_de_creation() {
        let own = layer_sources(
            discovery(Vec::new(), Some("the file has problems")),
            Some(discovery(Vec::new(), None)),
            "create the file",
        );
        assert_eq!(own.hint.as_deref(), Some("the file has problems"));
        let none = layer_sources(discovery(Vec::new(), None), None, "create the file");
        assert_eq!(none.hint.as_deref(), Some("create the file"));
    }
}
