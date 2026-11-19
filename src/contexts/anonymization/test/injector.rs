use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind};
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::open_pseudonymizer;
use crate::contexts::anonymization::injector::AnonymizationInjector;

#[test]
fn an_injector_over_an_unreadable_secret_builds_and_reports_the_store_unusable() {
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
fn an_injector_over_a_healthy_data_directory_exposes_its_use_cases() {
    let dir = tempfile::tempdir().unwrap();

    let injector = AnonymizationInjector::new(dir.path());

    assert!(injector.use_cases().is_ok());
}
