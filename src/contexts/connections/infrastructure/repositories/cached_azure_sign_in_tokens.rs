use std::sync::{Arc, Mutex};

use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::types::Secret;
use crate::contexts::connections::infrastructure::data_sources::azure::{
    AccessTokenFetcher, Clock, FetchedToken,
};

pub const EXPIRY_MARGIN_SECONDS: u64 = 300;

pub struct CachedAzureSignInTokens {
    fetcher: Arc<dyn AccessTokenFetcher>,
    clock: Arc<dyn Clock>,
    cached: Mutex<Option<FetchedToken>>,
}

impl CachedAzureSignInTokens {
    pub fn new(fetcher: Arc<dyn AccessTokenFetcher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            fetcher,
            clock,
            cached: Mutex::new(None),
        }
    }
}

impl CachedAzureSignInTokens {
    fn replace(&self, cached: &mut Option<FetchedToken>) -> Result<Secret, AccessTokenError> {
        match self.fetcher.fetch() {
            Ok(fetched) => {
                let token = fetched.token.clone();
                *cached = Some(fetched);
                Ok(token)
            }
            Err(error) => {
                *cached = None;
                Err(error)
            }
        }
    }
}

impl AzureSignInTokenProvider for CachedAzureSignInTokens {
    fn access_token(&self) -> Result<Secret, AccessTokenError> {
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let now = self.clock.now_epoch_seconds();
        if let Some(fetched) = cached.as_ref()
            && fetched.expires_at > now + EXPIRY_MARGIN_SECONDS
        {
            return Ok(fetched.token.clone());
        }
        self.replace(&mut cached)
    }

    fn renewed_access_token(&self) -> Result<Secret, AccessTokenError> {
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.replace(&mut cached)
    }
}

#[cfg(test)]
mod unit {
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

    use super::*;

    struct FakeClock(AtomicU64);

    impl Clock for FakeClock {
        fn now_epoch_seconds(&self) -> u64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    struct FakeFetcher {
        calls: AtomicUsize,
        lifetime: u64,
        clock: Arc<FakeClock>,
        failing: AtomicBool,
    }

    impl AccessTokenFetcher for FakeFetcher {
        fn fetch(&self) -> Result<FetchedToken, AccessTokenError> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.failing.load(Ordering::SeqCst) {
                return Err(AccessTokenError::SignInRequired {
                    detail: "ERROR: Please run 'az login'".into(),
                });
            }
            Ok(FetchedToken {
                token: Secret::new(format!("token-{call}")),
                expires_at: self.clock.now_epoch_seconds() + self.lifetime,
            })
        }
    }

    fn provider(lifetime: u64) -> (CachedAzureSignInTokens, Arc<FakeClock>, Arc<FakeFetcher>) {
        let clock = Arc::new(FakeClock(AtomicU64::new(1_000_000)));
        let fetcher = Arc::new(FakeFetcher {
            calls: AtomicUsize::new(0),
            lifetime,
            clock: clock.clone(),
            failing: AtomicBool::new(false),
        });
        (
            CachedAzureSignInTokens::new(fetcher.clone(), clock.clone()),
            clock,
            fetcher,
        )
    }

    fn advance(clock: &FakeClock, seconds: u64) {
        clock.0.fetch_add(seconds, Ordering::SeqCst);
    }

    #[test]
    fn un_jeton_encore_valide_est_reutilise_sans_nouvel_appel() {
        let (tokens, clock, fetcher) = provider(3_600);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-1");
        advance(&clock, 3_000);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-1");
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn un_jeton_proche_de_son_expiration_est_remplace_des_la_marge_atteinte() {
        let (tokens, clock, fetcher) = provider(3_600);
        tokens.access_token().unwrap();
        advance(&clock, 3_600 - EXPIRY_MARGIN_SECONDS - 1);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-1");
        advance(&clock, 1);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-2");
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn un_jeton_expire_est_remplace() {
        let (tokens, clock, _) = provider(3_600);
        tokens.access_token().unwrap();
        advance(&clock, 7_200);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-2");
    }

    #[test]
    fn un_jeton_dont_la_duree_est_inferieure_a_la_marge_n_est_jamais_reutilise() {
        let (tokens, _, fetcher) = provider(60);
        tokens.access_token().unwrap();
        tokens.access_token().unwrap();
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn un_renouvellement_force_ignore_le_cache_et_le_remplace() {
        let (tokens, _, fetcher) = provider(3_600);
        tokens.access_token().unwrap();
        assert_eq!(tokens.renewed_access_token().unwrap().expose(), "token-2");
        assert_eq!(tokens.access_token().unwrap().expose(), "token-2");
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn un_echec_n_est_pas_memorise_et_l_appel_suivant_reessaie() {
        let (tokens, clock, fetcher) = provider(3_600);
        tokens.access_token().unwrap();
        advance(&clock, 7_200);
        fetcher.failing.store(true, Ordering::SeqCst);
        assert!(matches!(
            tokens.access_token(),
            Err(AccessTokenError::SignInRequired { .. })
        ));
        fetcher.failing.store(false, Ordering::SeqCst);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-3");
    }
}
