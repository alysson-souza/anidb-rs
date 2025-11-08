//! Memory-based cache implementation
//!
//! This module provides an in-memory cache with configurable size limits and LRU eviction.

use crate::cache::traits::HashCache;
use crate::cache::{CacheEntry, CacheKey, CacheStats};
use anidb_client_core::error::Result;
use anidb_client_core::hashing::HashResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::time::interval;

/// Configuration for memory cache
#[derive(Debug, Clone)]
pub struct MemoryCacheConfig {
    /// Maximum number of entries to keep in cache
    pub max_entries: Option<usize>,
    /// Maximum total memory to use (in bytes)
    pub max_memory_bytes: Option<u64>,
    /// Default TTL for entries
    pub default_ttl: Duration,
    /// Interval for cleanup of expired entries
    pub cleanup_interval: Duration,
}

impl Default for MemoryCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: Some(10_000),
            max_memory_bytes: Some(100 * 1024 * 1024), // 100MB default
            default_ttl: Duration::from_secs(86400 * 30), // 30 days
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Memory-based cache for storing hash results
pub struct MemoryCache {
    entries: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    stats: Arc<RwLock<CacheStats>>,
    config: MemoryCacheConfig,
    shutdown: Arc<RwLock<bool>>,
}

impl MemoryCache {
    /// Create a new memory cache with default configuration
    pub fn new() -> Self {
        Self::with_config(MemoryCacheConfig::default())
    }

    /// Create a new memory cache with custom configuration
    pub fn with_config(config: MemoryCacheConfig) -> Self {
        let cache = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
            config: config.clone(),
            shutdown: Arc::new(RwLock::new(false)),
        };

        // Start background cleanup task
        let entries = cache.entries.clone();
        let stats = cache.stats.clone();
        let shutdown = cache.shutdown.clone();
        let cleanup_interval = config.cleanup_interval;

        tokio::spawn(async move {
            let mut ticker = interval(cleanup_interval);
            loop {
                ticker.tick().await;

                // Check if we should shutdown
                if *shutdown.read().await {
                    break;
                }

                // Clean up expired entries
                let mut entries = entries.write().await;
                let mut stats = stats.write().await;
                let now = SystemTime::now();

                entries.retain(|_, entry| {
                    if let Some(expires_at) = entry.expires_at
                        && now > expires_at
                    {
                        stats.entry_count -= 1;
                        return false;
                    }
                    true
                });
            }
        });

        cache
    }

    /// Perform LRU eviction to stay within limits
    async fn evict_if_needed(
        &self,
        entries: &mut HashMap<CacheKey, CacheEntry>,
        stats: &mut CacheStats,
    ) {
        // Check entry count limit
        if let Some(max) = self.config.max_entries {
            while entries.len() >= max {
                // Find and remove least recently used entry
                if let Some(oldest_key) = entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(k, _)| k.clone())
                {
                    entries.remove(&oldest_key);
                    stats.entry_count -= 1;
                }
            }
        }

        // Check memory limit
        if let Some(max_bytes) = self.config.max_memory_bytes {
            while stats.total_size_bytes > max_bytes && !entries.is_empty() {
                // Remove least recently used entry
                if let Some(oldest_key) = entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(k, _)| k.clone())
                    && let Some(removed) = entries.remove(&oldest_key)
                {
                    stats.entry_count -= 1;
                    // Estimate memory usage reduction
                    let estimated_size = std::mem::size_of::<CacheKey>() as u64
                        + std::mem::size_of::<CacheEntry>() as u64
                        + removed.hash_result.hash.len() as u64;
                    stats.total_size_bytes = stats.total_size_bytes.saturating_sub(estimated_size);
                }
            }
        }
    }

    /// Estimate memory usage for an entry
    fn estimate_entry_size(key: &CacheKey, value: &HashResult) -> u64 {
        std::mem::size_of::<CacheKey>() as u64
            + key.file_path.to_string_lossy().len() as u64
            + std::mem::size_of::<CacheEntry>() as u64
            + value.hash.len() as u64
    }
}

#[async_trait]
impl HashCache for MemoryCache {
    async fn get(&self, key: &CacheKey) -> Result<Option<HashResult>> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = entries.get_mut(key) {
            // Check if entry has expired
            if let Some(expires_at) = entry.expires_at
                && SystemTime::now() > expires_at
            {
                // Entry has expired, remove it
                let removed = entries.remove(key);
                if let Some(removed) = removed {
                    stats.entry_count -= 1;
                    let size = Self::estimate_entry_size(key, &removed.hash_result);
                    stats.total_size_bytes = stats.total_size_bytes.saturating_sub(size);
                }
                stats.miss_count += 1;
                return Ok(None);
            }

            // Update access metadata
            entry.last_accessed = SystemTime::now();
            entry.access_count += 1;
            stats.hit_count += 1;

            Ok(Some(entry.hash_result.clone()))
        } else {
            stats.miss_count += 1;
            Ok(None)
        }
    }

    async fn put(&self, key: &CacheKey, value: &HashResult) -> Result<()> {
        self.put_with_ttl(key, value, self.config.default_ttl).await
    }

    async fn put_with_ttl(&self, key: &CacheKey, value: &HashResult, ttl: Duration) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        // Evict if needed
        self.evict_if_needed(&mut entries, &mut stats).await;

        let now = SystemTime::now();
        let expires_at = Some(now + ttl);

        // Remove old entry if it exists
        if let Some(old_entry) = entries.remove(key) {
            let old_size = Self::estimate_entry_size(key, &old_entry.hash_result);
            stats.total_size_bytes = stats.total_size_bytes.saturating_sub(old_size);
            stats.entry_count -= 1;
        }

        let entry = CacheEntry {
            hash_result: value.clone(),
            created_at: now,
            last_accessed: now,
            access_count: 0,
            expires_at,
        };

        // Add new entry
        let new_size = Self::estimate_entry_size(key, value);
        entries.insert(key.clone(), entry);
        stats.entry_count += 1;
        stats.total_size_bytes += new_size;

        Ok(())
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        if let Some(removed) = entries.remove(key) {
            stats.entry_count -= 1;
            let size = Self::estimate_entry_size(key, &removed.hash_result);
            stats.total_size_bytes = stats.total_size_bytes.saturating_sub(size);
        }

        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        entries.clear();
        *stats = CacheStats::default();

        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
}

impl Drop for MemoryCache {
    fn drop(&mut self) {
        // Signal cleanup task to shutdown
        let shutdown = self.shutdown.clone();
        tokio::spawn(async move {
            *shutdown.write().await = true;
        });
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}
