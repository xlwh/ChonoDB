use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::hash::Hash;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub query: String,
    pub start_time: i64,
    pub end_time: i64,
    pub step: i64,
}

impl CacheKey {
    pub fn new(query: String, start_time: i64, end_time: i64, step: i64) -> Self {
        Self {
            query,
            start_time,
            end_time,
            step,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub ttl: Duration,
    pub access_count: u64,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T, ttl: Duration) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            ttl,
            access_count: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    pub fn access(&mut self) {
        self.access_count += 1;
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub size: usize,
    pub entry_count: usize,
}

impl CacheStats {
    pub fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            size: 0,
            entry_count: 0,
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_size: usize,
    pub ttl: Duration,
    pub enabled: bool,
    pub max_bytes: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            ttl: Duration::from_secs(300),
            enabled: true,
            max_bytes: 1024 * 1024 * 100, // 100MB
        }
    }
}

impl CacheConfig {
    pub fn new(entry_limit: usize, max_bytes: usize, ttl_secs: u64) -> Self {
        Self {
            max_size: entry_limit,
            ttl: Duration::from_secs(ttl_secs),
            enabled: true,
            max_bytes,
        }
    }
}

pub struct QueryCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone,
{
    cache: Arc<RwLock<HashMap<K, CacheEntry<V>>>>,
    config: CacheConfig,
    stats: Arc<RwLock<CacheStats>>,
    access_order: Arc<RwLock<Vec<K>>>,
}

impl<K, V> Clone for QueryCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
            access_order: Arc::clone(&self.access_order),
        }
    }
}

impl<K, V> QueryCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone,
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            config,
            stats: Arc::new(RwLock::new(CacheStats::new())),
            access_order: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        if !self.config.enabled {
            return None;
        }

        let mut cache = self.cache.write().unwrap();
        let mut stats = self.stats.write().unwrap();

        if let Some(entry) = cache.get_mut(key) {
            if entry.is_expired() {
                cache.remove(key);
                stats.misses += 1;
                stats.evictions += 1;
                return None;
            }

            entry.access();
            stats.hits += 1;

            let mut access_order = self.access_order.write().unwrap();
            access_order.retain(|k| k != key);
            access_order.push(key.clone());

            return Some(entry.value.clone());
        }

        stats.misses += 1;
        None
    }

    pub fn set(&self, key: K, value: V) {
        if !self.config.enabled {
            return;
        }

        let mut cache = self.cache.write().unwrap();
        let mut stats = self.stats.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();

        if cache.len() >= self.config.max_size {
            if let Some(evict_key) = access_order.first().cloned() {
                cache.remove(&evict_key);
                access_order.remove(0);
                stats.evictions += 1;
            }
        }

        let entry = CacheEntry::new(value, self.config.ttl);
        cache.insert(key.clone(), entry);
        access_order.push(key);
        stats.size = cache.len();
        stats.entry_count = cache.len();
    }

    pub fn remove(&self, key: &K) {
        let mut cache = self.cache.write().unwrap();
        let mut stats = self.stats.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();

        if cache.remove(key).is_some() {
            access_order.retain(|k| k != key);
            stats.size = cache.len();
            stats.entry_count = cache.len();
        }
    }

    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        let mut stats = self.stats.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();

        cache.clear();
        access_order.clear();
        stats.size = 0;
        stats.entry_count = 0;
    }

    pub fn stats(&self) -> CacheStats {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.read().unwrap().is_empty()
    }

    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.write().unwrap();
        let mut stats = self.stats.write().unwrap();
        let mut access_order = self.access_order.write().unwrap();

        let expired_keys: Vec<K> = cache
            .iter()
            .filter(|(_, entry)| entry.is_expired())
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired_keys {
            cache.remove(&key);
            access_order.retain(|k| k != &key);
            stats.evictions += 1;
        }

        stats.size = cache.len();
        stats.entry_count = cache.len();
    }
}

pub type ThreadSafeQueryCache<V> = QueryCache<CacheKey, V>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic() {
        let config = CacheConfig {
            max_size: 100,
            ttl: Duration::from_secs(10),
            enabled: true,
            max_bytes: 1024 * 1024, // 1MB
        };
        let cache: QueryCache<String, String> = QueryCache::new(config);

        cache.set("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.get(&"key2".to_string()), None);
    }

    #[test]
    fn test_cache_stats() {
        let config = CacheConfig::default();
        let cache: QueryCache<String, String> = QueryCache::new(config);

        cache.set("key1".to_string(), "value1".to_string());
        cache.get(&"key1".to_string());
        cache.get(&"key2".to_string());

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.size, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let config = CacheConfig {
            max_size: 2,
            ttl: Duration::from_secs(10),
            enabled: true,
            max_bytes: 1024 * 1024, // 1MB
        };
        let cache: QueryCache<String, String> = QueryCache::new(config);

        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());
        cache.set("key3".to_string(), "value3".to_string());

        assert_eq!(cache.get(&"key1".to_string()), None);
        assert_eq!(cache.get(&"key2".to_string()), Some("value2".to_string()));
        assert_eq!(cache.get(&"key3".to_string()), Some("value3".to_string()));
    }

    #[test]
    fn test_cache_expiration() {
        let config = CacheConfig {
            max_size: 100,
            ttl: Duration::from_millis(100),
            enabled: true,
            max_bytes: 1024 * 1024, // 1MB
        };
        let cache: QueryCache<String, String> = QueryCache::new(config);

        cache.set("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(cache.get(&"key1".to_string()), None);
    }

    #[test]
    fn test_cache_key() {
        let key1 = CacheKey::new("query1".to_string(), 1000, 2000, 0);
        let key2 = CacheKey::new("query1".to_string(), 1000, 2000, 0);
        let key3 = CacheKey::new("query2".to_string(), 1000, 2000, 0);

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }
}
