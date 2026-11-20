use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

pub type Slot<V> = Arc<Mutex<Option<V>>>;

type Key = (String, String);

struct Entry<V> {
    slot: Slot<V>,
    pinned: bool,
    last_used: u64,
}

pub struct SessionCache<V> {
    entries: HashMap<Key, Entry<V>>,
    clock: u64,
    max_unpinned_per_connection: usize,
}

impl<V> SessionCache<V> {
    pub fn new(max_unpinned_per_connection: usize) -> Self {
        Self {
            entries: HashMap::new(),
            clock: 0,
            max_unpinned_per_connection,
        }
    }

    pub fn slot(&mut self, connection: &str, database: &str, pinned: bool) -> Slot<V> {
        self.clock += 1;
        let key = (connection.to_string(), database.to_string());
        let entry = self.entries.entry(key).or_insert_with(|| Entry {
            slot: Slot::default(),
            pinned,
            last_used: 0,
        });
        entry.pinned |= pinned;
        entry.last_used = self.clock;
        let slot = entry.slot.clone();
        self.evict(connection);
        slot
    }

    pub fn forget(&mut self, connection: &str, database: &str, slot: &Slot<V>) {
        let key = (connection.to_string(), database.to_string());
        if self
            .entries
            .get(&key)
            .is_some_and(|entry| Arc::ptr_eq(&entry.slot, slot))
        {
            self.entries.remove(&key);
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn evict(&mut self, connection: &str) {
        loop {
            let mut unpinned: Vec<(&Key, u64)> = self
                .entries
                .iter()
                .filter(|(key, entry)| key.0 == connection && !entry.pinned)
                .map(|(key, entry)| (key, entry.last_used))
                .collect();
            if unpinned.len() <= self.max_unpinned_per_connection {
                return;
            }
            unpinned.sort_by_key(|(_, last_used)| *last_used);
            let oldest = unpinned[0].0.clone();
            self.entries.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod unit {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Probe(Arc<AtomicUsize>);

    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    fn open(cache: &mut SessionCache<Probe>, live: &Arc<AtomicUsize>, db: &str, pinned: bool) {
        let slot = cache.slot("shop", db, pinned);
        live.fetch_add(1, Ordering::SeqCst);
        *slot.try_lock().unwrap() = Some(Probe(live.clone()));
    }

    #[test]
    fn les_sessions_d_autres_bases_restent_bornees_et_la_base_par_defaut_survit() {
        let live = Arc::new(AtomicUsize::new(0));
        let mut cache = SessionCache::new(4);
        open(&mut cache, &live, "main", true);
        for index in 0..10 {
            open(&mut cache, &live, &format!("other_{index}"), false);
        }
        assert_eq!(live.load(Ordering::SeqCst), 5);
        assert_eq!(cache.len(), 5);
        assert!(
            cache
                .slot("shop", "main", true)
                .try_lock()
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn la_base_la_moins_recemment_utilisee_est_evincee_en_premier() {
        let live = Arc::new(AtomicUsize::new(0));
        let mut cache = SessionCache::new(2);
        open(&mut cache, &live, "a", false);
        open(&mut cache, &live, "b", false);
        cache.slot("shop", "a", false);
        open(&mut cache, &live, "c", false);
        assert!(cache.slot("shop", "a", false).try_lock().unwrap().is_some());
        assert!(cache.slot("shop", "b", false).try_lock().unwrap().is_none());
    }

    #[test]
    fn les_connexions_ne_s_evincent_pas_entre_elles() {
        let live = Arc::new(AtomicUsize::new(0));
        let mut cache = SessionCache::new(1);
        for connection in ["one", "two"] {
            for database in ["a", "b"] {
                let slot = cache.slot(connection, database, false);
                live.fetch_add(1, Ordering::SeqCst);
                *slot.try_lock().unwrap() = Some(Probe(live.clone()));
            }
        }
        assert_eq!(live.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn un_premier_echec_de_connexion_ne_laisse_aucune_place_occupee() {
        let mut cache: SessionCache<Probe> = SessionCache::new(4);
        let slot = cache.slot("shop", "main", true);
        cache.forget("shop", "main", &slot);
        assert_eq!(cache.len(), 0);
    }
}
