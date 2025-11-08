//! File-based cache implementation
//!
//! This module provides a file-based cache that persists hash results to disk.

use crate::cache::traits::HashCache;
use crate::cache::{CacheEntry, CacheKey, CacheStats};
use anidb_client_core::error::{Error, InternalError, Result};
use anidb_client_core::hashing::HashResult;
use async_trait::async_trait;
use std::collections::HashMap;
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

/// File-based cache for storing file processing results
pub struct FileCache {
    cache_dir: PathBuf,
    entries: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    stats: Arc<RwLock<CacheStats>>,
    max_entries: Option<usize>,
}

impl FileCache {
    /// Create a new file cache with cache directory path
    pub fn new(cache_dir: PathBuf) -> Result<Self> {
        // Ensure cache directory exists
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir).map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to create cache directory: {e}"
                )))
            })?;
        }

        // Load existing cache from disk if available
        let entries = Self::load_from_disk(&cache_dir).unwrap_or_default();
        let entry_count = entries.len();

        Ok(Self {
            cache_dir,
            entries: Arc::new(RwLock::new(entries)),
            stats: Arc::new(RwLock::new(CacheStats {
                entry_count,
                hit_count: 0,
                miss_count: 0,
                total_size_bytes: 0,
            })),
            max_entries: None,
        })
    }

    /// Create a cache with maximum entry limit
    pub fn with_max_entries(cache_dir: PathBuf, max_entries: usize) -> Result<Self> {
        let mut cache = Self::new(cache_dir)?;
        cache.max_entries = Some(max_entries);
        Ok(cache)
    }

    /// Store a hash result in the cache
    #[allow(dead_code)]
    pub async fn store_hash(&self, key: &CacheKey, hash_result: &HashResult) -> Result<()> {
        self.store_hash_with_ttl(key, hash_result, Duration::from_secs(86400 * 30))
            .await // 30 days default
    }

    /// Store a hash result with custom TTL
    #[allow(dead_code)]
    pub async fn store_hash_with_ttl(
        &self,
        key: &CacheKey,
        hash_result: &HashResult,
        ttl: Duration,
    ) -> Result<()> {
        self.put_with_ttl(key, hash_result, ttl).await
    }

    /// Get a hash result from the cache
    #[allow(dead_code)]
    pub async fn get_hash(&self, key: &CacheKey) -> Result<Option<HashResult>> {
        self.get(key).await
    }

    /// Get cache statistics (for backward compatibility)
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> Result<CacheStats> {
        self.stats().await
    }

    /// Load cache entries from disk
    fn load_from_disk(cache_dir: &Path) -> Result<HashMap<CacheKey, CacheEntry>> {
        let cache_file = cache_dir.join("cache.json");

        if !cache_file.exists() {
            return Ok(HashMap::new());
        }

        let data = std::fs::read_to_string(&cache_file).map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to read cache: {e}"
            )))
        })?;

        // Deserialize as Vec of tuples since CacheKey can't be JSON key
        let entries_vec: Vec<(CacheKey, CacheEntry)> =
            serde_json::from_str(&data).map_err(|e| {
                Error::Internal(InternalError::assertion(format!(
                    "Failed to parse cache: {e}"
                )))
            })?;

        // Filter out expired entries and convert to HashMap
        let now = SystemTime::now();
        let valid_entries: HashMap<CacheKey, CacheEntry> = entries_vec
            .into_iter()
            .filter(|(_, entry)| entry.expires_at.is_none_or(|expires_at| expires_at > now))
            .collect();

        Ok(valid_entries)
    }

    /// Save cache entries to disk
    async fn save_to_disk(&self, entries: &HashMap<CacheKey, CacheEntry>) -> Result<()> {
        let cache_file = self.cache_dir.join("cache.json");

        // Convert HashMap to Vec of tuples for serialization
        let entries_vec: Vec<(CacheKey, CacheEntry)> = entries
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let data = serde_json::to_string_pretty(&entries_vec).map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to serialize cache: {e}"
            )))
        })?;

        let mut file = fs::File::create(&cache_file).await.map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to create cache file: {e}"
            )))
        })?;

        file.write_all(data.as_bytes()).await.map_err(|e| {
            Error::Internal(InternalError::assertion(format!(
                "Failed to write cache: {e}"
            )))
        })?;

        Ok(())
    }
}

#[async_trait]
impl HashCache for FileCache {
    async fn get(&self, key: &CacheKey) -> Result<Option<HashResult>> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        if let Some(entry) = entries.get_mut(key) {
            // Check if entry has expired
            if let Some(expires_at) = entry.expires_at
                && SystemTime::now() > expires_at
            {
                // Entry has expired, remove it
                entries.remove(key);
                stats.entry_count -= 1;
                stats.miss_count += 1;
                self.save_to_disk(&entries).await?;
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
        self.put_with_ttl(key, value, Duration::from_secs(86400 * 30))
            .await // 30 days default
    }

    async fn put_with_ttl(&self, key: &CacheKey, value: &HashResult, ttl: Duration) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        // Check if we need to evict entries
        if let Some(max) = self.max_entries {
            while entries.len() >= max {
                // Simple LRU eviction - remove oldest entry
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

        let now = SystemTime::now();
        let expires_at = Some(now.add(ttl));

        let entry = CacheEntry {
            hash_result: value.clone(),
            created_at: now,
            last_accessed: now,
            access_count: 0,
            expires_at,
        };

        entries.insert(key.clone(), entry);
        stats.entry_count += 1;

        // Save to disk
        self.save_to_disk(&entries).await?;

        Ok(())
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        if entries.remove(key).is_some() {
            stats.entry_count -= 1;
            self.save_to_disk(&entries).await?;
        }

        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        let mut entries = self.entries.write().await;
        let mut stats = self.stats.write().await;

        entries.clear();
        stats.entry_count = 0;
        stats.hit_count = 0;
        stats.miss_count = 0;
        stats.total_size_bytes = 0;

        self.save_to_disk(&entries).await?;

        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }
}

impl Default for FileCache {
    fn default() -> Self {
        // Use default XDG data directory for cache
        let cache_dir = dirs::data_dir()
            .map(|d| d.join("anidb/cache"))
            .unwrap_or_else(|| PathBuf::from(".anidb/cache"));
        Self::new(cache_dir).unwrap()
    }
}
