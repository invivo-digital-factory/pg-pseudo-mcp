use std::sync::{Arc, Mutex, MutexGuard};

use crate::contexts::connections::domain::errors::AccessTokenError;
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::types::Secret;
use crate::contexts::connections::infrastructure::data_sources::azure::{
    AccessTokenFetcher, Clock, FetchedToken,
};

pub const EXPIRY_MARGIN_SECONDS: u64 = 300;
pub const FAILURE_MEMORY_SECONDS: u64 = 30;

#[derive(Default)]
struct State {
    token: Option<FetchedToken>,
    failure: Option<RememberedFailure>,
}

struct RememberedFailure {
    error: AccessTokenError,
    until: u64,
}

pub struct CachedAzureSignInTokens {
    fetcher: Arc<dyn AccessTokenFetcher>,
    clock: Arc<dyn Clock>,
    state: Mutex<State>,
}

impl CachedAzureSignInTokens {
    pub fn new(fetcher: Arc<dyn AccessTokenFetcher>, clock: Arc<dyn Clock>) -> Self {
        Self {
            fetcher,
            clock,
            state: Mutex::new(State::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn replace(&self, state: &mut State, now: u64) -> Result<Secret, AccessTokenError> {
        match self.fetcher.fetch() {
            Ok(fetched) => {
                let token = fetched.token.clone();
                state.token = Some(fetched);
                state.failure = None;
                Ok(token)
            }
            Err(error) => {
                state.failure = Some(RememberedFailure {
                    error: error.clone(),
                    until: now + FAILURE_MEMORY_SECONDS,
                });
                Err(error)
            }
        }
    }
}

impl AzureSignInTokenProvider for CachedAzureSignInTokens {
    fn access_token(&self) -> Result<Secret, AccessTokenError> {
        let mut state = self.lock();
        let now = self.clock.now_epoch_seconds();
        if let Some(fetched) = state.token.as_ref()
            && fetched.expires_at > now + EXPIRY_MARGIN_SECONDS
        {
            return Ok(fetched.token.clone());
        }
        if let Some(failure) = state.failure.as_ref()
            && now < failure.until
        {
            return Err(failure.error.clone());
        }
        self.replace(&mut state, now)
    }

    fn renewed_access_token(&self) -> Result<Secret, AccessTokenError> {
        let mut state = self.lock();
        let now = self.clock.now_epoch_seconds();
        self.replace(&mut state, now)
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
                return Err(AccessTokenError::SignInRequired);
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
    fn un_echec_est_memorise_pendant_la_fenetre_puis_l_appel_suivant_reessaie() {
        let (tokens, clock, fetcher) = provider(3_600);
        tokens.access_token().unwrap();
        advance(&clock, 7_200);
        fetcher.failing.store(true, Ordering::SeqCst);
        assert!(matches!(
            tokens.access_token(),
            Err(AccessTokenError::SignInRequired)
        ));
        fetcher.failing.store(false, Ordering::SeqCst);
        advance(&clock, FAILURE_MEMORY_SECONDS - 1);
        assert!(matches!(
            tokens.access_token(),
            Err(AccessTokenError::SignInRequired)
        ));
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 2);
        advance(&clock, 1);
        assert_eq!(tokens.access_token().unwrap().expose(), "token-3");
    }

    #[test]
    fn un_renouvellement_force_ignore_l_echec_memorise() {
        let (tokens, _, fetcher) = provider(3_600);
        fetcher.failing.store(true, Ordering::SeqCst);
        assert!(tokens.access_token().is_err());
        fetcher.failing.store(false, Ordering::SeqCst);
        assert_eq!(tokens.renewed_access_token().unwrap().expose(), "token-2");
        assert_eq!(tokens.access_token().unwrap().expose(), "token-2");
    }

    #[test]
    fn un_renouvellement_force_en_echec_garde_le_jeton_encore_valide() {
        let (tokens, _, fetcher) = provider(3_600);
        tokens.access_token().unwrap();
        fetcher.failing.store(true, Ordering::SeqCst);
        assert!(tokens.renewed_access_token().is_err());
        assert_eq!(tokens.access_token().unwrap().expose(), "token-1");
        assert_eq!(fetcher.calls.load(Ordering::SeqCst), 2);
    }
}
