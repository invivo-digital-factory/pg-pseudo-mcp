use std::path::Path;
use std::sync::Arc;

use crate::common::AppConfig;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::GeneratorConfig;
use crate::contexts::anonymization::infrastructure::data_sources::data_directory::{
    data_dir, open_pseudonymizer,
};
use crate::contexts::anonymization::infrastructure::repositories::file_mapping_purger::FileMappingPurger;
use crate::contexts::anonymization::presentation::mapping_purge::MappingPurgeHandler;
use crate::contexts::anonymization::use_cases::anonymize_free_text::AnonymizeFreeText;
use crate::contexts::anonymization::use_cases::anonymize_message::AnonymizeMessage;
use crate::contexts::anonymization::use_cases::anonymize_plan::AnonymizePlan;
use crate::contexts::anonymization::use_cases::anonymize_result_set::AnonymizeResultSet;
use crate::contexts::anonymization::use_cases::classify_column::ClassifyColumn;
use crate::contexts::anonymization::use_cases::dictionary_kind::DictionaryKind;
use crate::contexts::anonymization::use_cases::purge_mapping::PurgeMapping;
use crate::contexts::anonymization::use_cases::translate_pseudonym::TranslatePseudonym;

#[derive(Clone)]
pub struct AnonymizationUseCases {
    pub anonymize_result_set: AnonymizeResultSet,
    pub anonymize_free_text: AnonymizeFreeText,
    pub anonymize_message: AnonymizeMessage,
    pub anonymize_plan: AnonymizePlan,
    pub translate_pseudonym: TranslatePseudonym,
    pub classify_column: ClassifyColumn,
    pub dictionary_kind: DictionaryKind,
}

pub struct AnonymizationInjector {
    use_cases: Result<AnonymizationUseCases, String>,
}

impl AnonymizationInjector {
    pub fn new(data_dir: &Path) -> Self {
        Self::with_config(data_dir, GeneratorConfig::default())
    }

    pub fn with_config(data_dir: &Path, config: GeneratorConfig) -> Self {
        let use_cases = open_pseudonymizer(data_dir, config)
            .map(|pseudonymizer| use_cases_over(Arc::new(pseudonymizer)))
            .map_err(|error| error.to_string());
        Self::keeping(use_cases)
    }

    pub fn from_config(config: &AppConfig) -> Self {
        match data_dir(config) {
            Ok(dir) => Self::new(&dir),
            Err(error) => Self::keeping(Err(error.to_string())),
        }
    }

    pub fn use_cases(&self) -> Result<AnonymizationUseCases, String> {
        self.use_cases.clone()
    }

    fn keeping(use_cases: Result<AnonymizationUseCases, String>) -> Self {
        if let Err(error) = &use_cases {
            tracing::error!(%error, "pseudonym store unavailable; data tools will refuse to answer");
        }
        Self { use_cases }
    }
}

fn use_cases_over(pseudonymizer: Arc<Pseudonymizer>) -> AnonymizationUseCases {
    AnonymizationUseCases {
        anonymize_result_set: AnonymizeResultSet::new(Arc::clone(&pseudonymizer)),
        anonymize_free_text: AnonymizeFreeText::new(Arc::clone(&pseudonymizer)),
        anonymize_message: AnonymizeMessage::new(Arc::clone(&pseudonymizer)),
        anonymize_plan: AnonymizePlan::new(Arc::clone(&pseudonymizer)),
        translate_pseudonym: TranslatePseudonym::new(pseudonymizer),
        classify_column: ClassifyColumn,
        dictionary_kind: DictionaryKind,
    }
}

pub fn mapping_purge_handler(config: &AppConfig) -> Result<MappingPurgeHandler, String> {
    let dir = data_dir(config).map_err(|error| error.to_string())?;
    let purge_mapping = PurgeMapping::new(Arc::new(FileMappingPurger::new(&dir)));
    Ok(MappingPurgeHandler::new(purge_mapping, dir))
}
