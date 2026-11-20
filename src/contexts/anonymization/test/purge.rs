use std::path::PathBuf;
use std::sync::Arc;

use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;
use crate::contexts::anonymization::infrastructure::repositories::file_mapping_purger::FileMappingPurger;
use crate::contexts::anonymization::injector::AnonymizationInjector;
use crate::contexts::anonymization::use_cases::purge_mapping::PurgeMapping;

mod integration {
    use super::*;

    #[test]
    fn la_purge_supprime_le_stockage_ses_journaux_et_le_secret() {
        let dir = tempfile::tempdir().unwrap();
        let pseudonymizer = open_pseudonymizer(dir.path(), GeneratorConfig::default()).unwrap();
        pseudonymizer.pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr");
        let store = dir.path().join("mapping.sqlite");
        let candidates: Vec<PathBuf> = [
            store.clone(),
            dir.path().join("mapping.sqlite-wal"),
            dir.path().join("mapping.sqlite-shm"),
            dir.path().join("secret"),
        ]
        .into_iter()
        .filter(|path| path.exists())
        .collect();
        assert!(candidates.contains(&store));
        assert!(candidates.contains(&dir.path().join("secret")));

        let deleted = PurgeMapping::new(Arc::new(FileMappingPurger::new(dir.path())))
            .execute()
            .unwrap();

        let mut expected = candidates.clone();
        expected.sort();
        let mut actual = deleted.clone();
        actual.sort();
        assert_eq!(actual, expected);
        for path in &candidates {
            assert!(!path.exists(), "{}", path.display());
        }
    }

    #[test]
    fn un_pseudonyme_emis_avant_la_purge_ne_se_traduit_plus() {
        let dir = tempfile::tempdir().unwrap();
        let before = open_pseudonymizer(dir.path(), GeneratorConfig::default())
            .unwrap()
            .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr");
        let translate = AnonymizationInjector::new(dir.path())
            .use_cases()
            .unwrap()
            .translate_pseudonym;
        assert_eq!(
            translate.execute(&before.text).real,
            Some((Kind::Email, "ylwenna.kervadec@canary-mail.fr".to_string()))
        );

        PurgeMapping::new(Arc::new(FileMappingPurger::new(dir.path())))
            .execute()
            .unwrap();

        let after = AnonymizationInjector::new(dir.path())
            .use_cases()
            .unwrap()
            .translate_pseudonym;
        assert_eq!(after.execute(&before.text).real, None);
    }
}
