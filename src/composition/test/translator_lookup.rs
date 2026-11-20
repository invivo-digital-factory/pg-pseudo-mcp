use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::composition::adapters::PseudonymTranslatorAdapter;
use crate::contexts::anonymization::domain::errors::PseudoError;
use crate::contexts::anonymization::domain::repositories::PseudonymStore;
use crate::contexts::anonymization::domain::services::keys::Keys;
use crate::contexts::anonymization::domain::services::pseudonymizer::Pseudonymizer;
use crate::contexts::anonymization::domain::types::{GeneratorConfig, Kind, StoreOutcome};
use crate::contexts::anonymization::use_cases::translate_pseudonym::TranslatePseudonym;
use crate::contexts::querying::domain::repositories::PseudonymTranslator;

struct DegradedStore {
    lookups: Arc<AtomicUsize>,
}

impl PseudonymStore for DegradedStore {
    fn get_or_create(
        &self,
        _kind: Kind,
        _forward_key: &[u8; 32],
        _max_draws: usize,
        _draw: &mut dyn FnMut() -> String,
        _encrypt: &dyn Fn() -> Result<Vec<u8>, PseudoError>,
    ) -> Result<StoreOutcome, PseudoError> {
        Err(PseudoError::Store("degraded".into()))
    }

    fn find(&self, _pseudonym: &str) -> Result<Vec<(String, Vec<u8>)>, PseudoError> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        Err(PseudoError::Store("degraded".into()))
    }
}

fn adapter_over_degraded_store() -> (PseudonymTranslatorAdapter, Arc<AtomicUsize>) {
    let lookups = Arc::new(AtomicUsize::new(0));
    let store = DegradedStore {
        lookups: Arc::clone(&lookups),
    };
    let pseudonymizer = Pseudonymizer::new(
        Keys::new([7; 32]),
        Box::new(store),
        GeneratorConfig::default(),
    );
    let adapter = PseudonymTranslatorAdapter::new(TranslatePseudonym::new(Arc::new(pseudonymizer)));
    (adapter, lookups)
}

mod unit {
    use super::*;

    #[test]
    fn le_magasin_est_consulte_une_seule_fois_par_litteral_inconnu() {
        for literal in ["ylwenna.kervadec@canary-mail.fr", "hello"] {
            let (adapter, lookups) = adapter_over_degraded_store();

            let _ = adapter.lookup(literal);

            assert_eq!(lookups.load(Ordering::SeqCst), 1, "{literal}");
        }
    }
}
