use lru_cache::LruCache;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SharedLruCache<K: Eq + Hash, V: Clone>(Arc<Mutex<LruCache<K, V>>>);

impl<K: Eq + Hash, V: Clone> SharedLruCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self(Arc::new(Mutex::new(LruCache::new(capacity))))
    }

    pub fn insert(&self, key: K, value: V) {
        self.0.lock().unwrap().insert(key, value);
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.0.lock().unwrap().get_mut(&key).map(|a| a.clone())
    }
}
