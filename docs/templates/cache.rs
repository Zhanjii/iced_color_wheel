//! cache.rs
//!
//! General-purpose caching utilities with TTL and LRU eviction.
//!
//! # Example
//!
//! ```rust
//! use cache::{TimedCache, timed_cache, get_cache_manager};
//!
//! // Direct cache usage
//! let mut cache = TimedCache::new(300, 128);
//! cache.set("key".to_string(), "value".to_string());
//! if let Some(value) = cache.get("key") {
//!     println!("Cached: {}", value);
//! }
//!
//! // Central cache management
//! let manager = get_cache_manager();
//! let api_cache = manager.get_cache("api", 300, 128);
//! ```

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info};

// =============================================================================
// CACHE ENTRY
// =============================================================================

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    created_at: Instant,
    last_accessed: Instant,
    access_count: u64,
}

// =============================================================================
// TIMED CACHE
// =============================================================================

/// Thread-safe cache with time-based expiration and LRU eviction.
#[derive(Debug)]
pub struct TimedCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    ttl: Duration,
    maxsize: usize,
    name: String,
    cache: Arc<Mutex<HashMap<K, CacheEntry<V>>>>,
    insertion_order: Arc<Mutex<Vec<K>>>,
    hits: Arc<Mutex<u64>>,
    misses: Arc<Mutex<u64>>,
}

impl<K, V> TimedCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    /// Creates a new timed cache.
    pub fn new(ttl_seconds: u64, maxsize: usize) -> Self {
        Self::with_name(ttl_seconds, maxsize, "TimedCache")
    }

    /// Creates a new timed cache with a custom name.
    pub fn with_name(ttl_seconds: u64, maxsize: usize, name: impl Into<String>) -> Self {
        Self {
            ttl: Duration::from_secs(ttl_seconds),
            maxsize,
            name: name.into(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            insertion_order: Arc::new(Mutex::new(Vec::new())),
            hits: Arc::new(Mutex::new(0)),
            misses: Arc::new(Mutex::new(0)),
        }
    }

    /// Gets a value from cache if it exists and is not expired.
    pub fn get(&self, key: &K) -> Option<V> {
        let mut cache = self.cache.lock().unwrap();
        let now = Instant::now();

        if let Some(entry) = cache.get_mut(key) {
            // Check if expired
            if now.duration_since(entry.created_at) > self.ttl {
                cache.remove(key);
                *self.misses.lock().unwrap() += 1;
                debug!("[{}] Cache miss (expired)", self.name);
                return None;
            }

            // Update access tracking
            entry.last_accessed = now;
            entry.access_count += 1;
            *self.hits.lock().unwrap() += 1;
            debug!("[{}] Cache hit", self.name);
            return Some(entry.value.clone());
        }

        *self.misses.lock().unwrap() += 1;
        None
    }

    /// Sets a value in the cache.
    pub fn set(&self, key: K, value: V) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.insertion_order.lock().unwrap();
        let now = Instant::now();

        // Remove oldest entries if at capacity
        while cache.len() >= self.maxsize && !cache.is_empty() {
            if let Some(oldest_key) = order.first().cloned() {
                cache.remove(&oldest_key);
                order.remove(0);
                debug!("[{}] Evicted LRU entry", self.name);
            }
        }

        // Add or update entry
        cache.insert(
            key.clone(),
            CacheEntry {
                value,
                created_at: now,
                last_accessed: now,
                access_count: 0,
            },
        );

        // Track insertion order
        if !order.contains(&key) {
            order.push(key.clone());
        }

        debug!("[{}] Cached entry", self.name);
    }

    /// Removes a specific entry from cache.
    pub fn invalidate(&self, key: &K) -> bool {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.insertion_order.lock().unwrap();

        if cache.remove(key).is_some() {
            order.retain(|k| k != key);
            debug!("[{}] Invalidated entry", self.name);
            true
        } else {
            false
        }
    }

    /// Clears all cache entries.
    pub fn clear(&self) -> usize {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.insertion_order.lock().unwrap();
        let count = cache.len();

        cache.clear();
        order.clear();
        *self.hits.lock().unwrap() = 0;
        *self.misses.lock().unwrap() = 0;

        info!("[{}] Cleared {} entries", self.name, count);
        count
    }

    /// Removes all expired entries.
    pub fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.insertion_order.lock().unwrap();
        let now = Instant::now();

        let expired_keys: Vec<K> = cache
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.created_at) > self.ttl)
            .map(|(k, _)| k.clone())
            .collect();

        for key in &expired_keys {
            cache.remove(key);
            order.retain(|k| k != key);
        }

        let count = expired_keys.len();
        if count > 0 {
            debug!("[{}] Cleaned up {} expired entries", self.name, count);
        }

        count
    }

    /// Gets cache statistics.
    pub fn get_stats(&self) -> CacheStats {
        let cache = self.cache.lock().unwrap();
        let hits = *self.hits.lock().unwrap();
        let misses = *self.misses.lock().unwrap();
        let total_requests = hits + misses;
        let hit_rate = if total_requests > 0 {
            (hits as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        CacheStats {
            name: self.name.clone(),
            size: cache.len(),
            maxsize: self.maxsize,
            ttl_seconds: self.ttl.as_secs(),
            hits,
            misses,
            hit_rate_percent: hit_rate,
        }
    }

    /// Returns number of entries in cache.
    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    /// Checks if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.lock().unwrap().is_empty()
    }
}

impl<K, V> Clone for TimedCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            ttl: self.ttl,
            maxsize: self.maxsize,
            name: self.name.clone(),
            cache: Arc::clone(&self.cache),
            insertion_order: Arc::clone(&self.insertion_order),
            hits: Arc::clone(&self.hits),
            misses: Arc::clone(&self.misses),
        }
    }
}

// =============================================================================
// CACHE STATS
// =============================================================================

/// Cache statistics for monitoring.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub name: String,
    pub size: usize,
    pub maxsize: usize,
    pub ttl_seconds: u64,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate_percent: f64,
}

// =============================================================================
// CACHE MANAGER
// =============================================================================

/// Central manager for multiple named caches.
#[derive(Debug)]
pub struct CacheManager {
    caches: Arc<Mutex<HashMap<String, TimedCache<String, String>>>>,
}

impl CacheManager {
    /// Creates a new cache manager.
    pub fn new() -> Self {
        Self {
            caches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Gets or creates a named cache.
    pub fn get_cache(&self, name: &str, ttl_seconds: u64, maxsize: usize) -> TimedCache<String, String> {
        let mut caches = self.caches.lock().unwrap();

        caches
            .entry(name.to_string())
            .or_insert_with(|| {
                info!("Created cache: {} (ttl={}s, max={})", name, ttl_seconds, maxsize);
                TimedCache::with_name(ttl_seconds, maxsize, name)
            })
            .clone()
    }

    /// Clears all managed caches.
    pub fn clear_all(&self) -> HashMap<String, usize> {
        let caches = self.caches.lock().unwrap();
        let mut results = HashMap::new();

        for (name, cache) in caches.iter() {
            results.insert(name.clone(), cache.clear());
        }

        info!("Cleared all caches: {:?}", results);
        results
    }

    /// Gets statistics for all managed caches.
    pub fn get_all_stats(&self) -> HashMap<String, CacheStats> {
        let caches = self.caches.lock().unwrap();
        caches
            .iter()
            .map(|(name, cache)| (name.clone(), cache.get_stats()))
            .collect()
    }

    /// Cleans up expired entries in all caches.
    pub fn cleanup_all_expired(&self) -> HashMap<String, usize> {
        let caches = self.caches.lock().unwrap();
        caches
            .iter()
            .map(|(name, cache)| (name.clone(), cache.cleanup_expired()))
            .collect()
    }
}

impl Default for CacheManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global cache manager
static CACHE_MANAGER: std::sync::OnceLock<CacheManager> = std::sync::OnceLock::new();

/// Gets the global CacheManager instance.
pub fn get_cache_manager() -> &'static CacheManager {
    CACHE_MANAGER.get_or_init(CacheManager::new)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_cache_set_get() {
        let cache = TimedCache::new(60, 10);
        cache.set("key1".to_string(), "value1".to_string());

        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));
        assert_eq!(cache.get(&"key2".to_string()), None);
    }

    #[test]
    fn test_cache_expiration() {
        let cache = TimedCache::new(1, 10);
        cache.set("key".to_string(), "value".to_string());

        assert_eq!(cache.get(&"key".to_string()), Some("value".to_string()));

        thread::sleep(Duration::from_secs(2));
        assert_eq!(cache.get(&"key".to_string()), None);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = TimedCache::new(60, 2);
        cache.set("key1".to_string(), "value1".to_string());
        cache.set("key2".to_string(), "value2".to_string());
        cache.set("key3".to_string(), "value3".to_string());

        // First key should be evicted
        assert_eq!(cache.get(&"key1".to_string()), None);
        assert_eq!(cache.get(&"key2".to_string()), Some("value2".to_string()));
        assert_eq!(cache.get(&"key3".to_string()), Some("value3".to_string()));
    }

    #[test]
    fn test_cache_stats() {
        let cache = TimedCache::new(60, 10);
        cache.set("key".to_string(), "value".to_string());

        cache.get(&"key".to_string());
        cache.get(&"missing".to_string());

        let stats = cache.get_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }
}
