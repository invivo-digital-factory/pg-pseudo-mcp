use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;
use crate::contexts::anonymization::injector::AnonymizationInjector;

mod integration {
    use super::*;

    #[test]
    fn un_injecteur_sur_un_secret_illisible_signale_le_stockage_inutilisable() {
        let dir = tempfile::tempdir().unwrap();
        open_pseudonymizer(dir.path(), GeneratorConfig::default())
            .unwrap()
            .pseudonymize(Kind::Email, "ylwenna.kervadec@canary-mail.fr");
        std::fs::remove_file(dir.path().join("secret")).unwrap();

        let injector = AnonymizationInjector::new(dir.path());

        let error = match injector.use_cases() {
            Ok(_) => panic!("the store must be reported unusable"),
            Err(error) => error,
        };
        assert!(error.contains("mapping purge"), "{error}");
    }

    #[test]
    fn un_injecteur_sur_un_repertoire_sain_expose_ses_cas_d_usage() {
        let dir = tempfile::tempdir().unwrap();

        let injector = AnonymizationInjector::new(dir.path());

        assert!(injector.use_cases().is_ok());
    }
}
