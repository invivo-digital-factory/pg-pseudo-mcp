use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::contexts::connections::domain::errors::PasswordCommandError;
use crate::contexts::connections::domain::repositories::CommandPasswordProvider;
use crate::contexts::connections::domain::types::Secret;
use crate::contexts::connections::infrastructure::data_sources::password_command::PasswordCommandRunner;

type Key = (String, Vec<String>);

pub struct CachedCommandPasswords {
    runner: Arc<dyn PasswordCommandRunner>,
    passwords: Mutex<HashMap<Key, Secret>>,
}

impl CachedCommandPasswords {
    pub fn new(runner: Arc<dyn PasswordCommandRunner>) -> Self {
        Self {
            runner,
            passwords: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Key, Secret>> {
        self.passwords
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn run_and_keep(
        &self,
        passwords: &mut HashMap<Key, Secret>,
        key: Key,
    ) -> Result<Secret, PasswordCommandError> {
        let secret = self.runner.run(&key.1)?;
        passwords.insert(key, secret.clone());
        Ok(secret)
    }
}

impl CommandPasswordProvider for CachedCommandPasswords {
    fn password(&self, connection: &str, argv: &[String]) -> Result<Secret, PasswordCommandError> {
        let key = (connection.to_string(), argv.to_vec());
        let mut passwords = self.lock();
        if let Some(secret) = passwords.get(&key) {
            return Ok(secret.clone());
        }
        self.run_and_keep(&mut passwords, key)
    }

    fn renewed_password(
        &self,
        connection: &str,
        argv: &[String],
    ) -> Result<Secret, PasswordCommandError> {
        let key = (connection.to_string(), argv.to_vec());
        let mut passwords = self.lock();
        passwords.remove(&key);
        self.run_and_keep(&mut passwords, key)
    }
}

#[cfg(test)]
mod unit {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Counting {
        runs: AtomicUsize,
        fail_from: usize,
    }

    impl Counting {
        fn new(fail_from: usize) -> Arc<Self> {
            Arc::new(Self {
                runs: AtomicUsize::new(0),
                fail_from,
            })
        }

        fn runs(&self) -> usize {
            self.runs.load(Ordering::SeqCst)
        }
    }

    impl PasswordCommandRunner for Counting {
        fn run(&self, argv: &[String]) -> Result<Secret, PasswordCommandError> {
            let run = self.runs.fetch_add(1, Ordering::SeqCst) + 1;
            if run >= self.fail_from {
                return Err(PasswordCommandError::ExitStatus(1));
            }
            Ok(Secret::new(format!("{}-{run}", argv[0])))
        }
    }

    fn argv(program: &str) -> Vec<String> {
        vec![program.to_string(), "--flag".to_string()]
    }

    #[test]
    fn deux_demandes_pour_la_meme_connexion_et_la_meme_commande_ne_lancent_la_commande_qu_une_fois()
    {
        let runner = Counting::new(usize::MAX);
        let passwords = CachedCommandPasswords::new(runner.clone());
        let first = passwords.password("app", &argv("cmd")).unwrap();
        let second = passwords.password("app", &argv("cmd")).unwrap();
        assert_eq!(first, second);
        assert_eq!(runner.runs(), 1);
    }

    #[test]
    fn une_commande_modifiee_ou_une_autre_connexion_relance_la_commande() {
        let runner = Counting::new(usize::MAX);
        let passwords = CachedCommandPasswords::new(runner.clone());
        passwords.password("app", &argv("cmd")).unwrap();
        passwords.password("app", &argv("other")).unwrap();
        passwords.password("billing", &argv("cmd")).unwrap();
        assert_eq!(runner.runs(), 3);
    }

    #[test]
    fn un_mot_de_passe_renouvele_remplace_celui_en_memoire_avec_une_seule_execution() {
        let runner = Counting::new(usize::MAX);
        let passwords = CachedCommandPasswords::new(runner.clone());
        let old = passwords.password("app", &argv("cmd")).unwrap();
        let renewed = passwords.renewed_password("app", &argv("cmd")).unwrap();
        assert_ne!(old, renewed);
        assert_eq!(runner.runs(), 2);
        assert_eq!(passwords.password("app", &argv("cmd")).unwrap(), renewed);
        assert_eq!(runner.runs(), 2);
    }

    #[test]
    fn un_echec_n_est_pas_memorise_et_la_commande_est_relancee_a_la_demande_suivante() {
        let runner = Counting::new(1);
        let passwords = CachedCommandPasswords::new(runner.clone());
        assert_eq!(
            passwords.password("app", &argv("cmd")),
            Err(PasswordCommandError::ExitStatus(1))
        );
        assert_eq!(
            passwords.password("app", &argv("cmd")),
            Err(PasswordCommandError::ExitStatus(1))
        );
        assert_eq!(runner.runs(), 2);
    }
}
