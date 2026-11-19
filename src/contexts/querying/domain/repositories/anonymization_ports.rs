use std::sync::Arc;

use super::{PersonalColumnDictionary, PseudonymTranslator, ResultAnonymizer};

#[derive(Clone)]
pub struct AnonymizationPorts {
    pub translator: Arc<dyn PseudonymTranslator>,
    pub anonymizer: Arc<dyn ResultAnonymizer>,
    pub dictionary: Arc<dyn PersonalColumnDictionary>,
}
