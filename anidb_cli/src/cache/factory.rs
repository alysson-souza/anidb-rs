//! Cache factory for creating different cache implementations
//!
//! This module provides a factory pattern for creating cache instances
//! based on configuration.

use crate::cache::memory_cache::MemoryCacheConfig;
use crate::cache::traits::HashCache;
use crate::cache::{file_cache::FileCache, memory_cache::MemoryCache, noop_cache::NoOpCache};
use crate::paths;
use anidb_client_core::error::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// Configuration for different cache types
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CacheConfig {
    /// File-based cache with optional max entries
    File {
        cache_dir: PathBuf,
        max_entries: Option<usize>,
    },
    /// Memory-based cache with configuration
    Memory(MemoryCacheConfig),
    /// No-operation cache (no caching)
    NoOp,
    /// Layered cache (memory + file fallback)
    Layered {
        memory_config: MemoryCacheConfig,
        cache_dir: PathBuf,
    },
}

impl Default for CacheConfig {
    fn default() -> Self {
        // Use centralized cache directory
        let cache_dir = paths::get_cache_dir();
        Self::File {
            cache_dir,
            max_entries: None,
        }
    }
}

/// Factory for creating cache implementations
pub struct CacheFactory;

impl CacheFactory {
    /// Create a cache implementation based on configuration
    pub fn create(config: CacheConfig) -> Result<Arc<dyn HashCache>> {
        match config {
            CacheConfig::File {
                cache_dir,
                max_entries,
            } => {
                let cache = if let Some(max) = max_entries {
                    FileCache::with_max_entries(cache_dir, max)?
                } else {
                    FileCache::new(cache_dir)?
                };
                Ok(Arc::new(cache))
            }
            CacheConfig::Memory(config) => {
                let cache = MemoryCache::with_config(config);
                Ok(Arc::new(cache))
            }
            CacheConfig::NoOp => Ok(Arc::new(NoOpCache::new())),
            CacheConfig::Layered {
                memory_config,
                cache_dir,
            } => {
                // Create layered cache with memory as L1 and file as L2
                let memory_cache = MemoryCache::with_config(memory_config);
                let file_cache = FileCache::new(cache_dir)?;
                let layered = LayeredCache::new(memory_cache, file_cache);
                Ok(Arc::new(layered))
            }
        }
    }

    /// Create a file-based cache
    pub fn file(cache_dir: PathBuf) -> Result<Arc<dyn HashCache>> {
        Self::create(CacheConfig::File {
            cache_dir,
            max_entries: None,
        })
    }

    /// Create a memory-based cache
    #[allow(dead_code)]
    pub fn memory() -> Result<Arc<dyn HashCache>> {
        Self::create(CacheConfig::Memory(MemoryCacheConfig::default()))
    }

    /// Create a no-op cache
    pub fn noop() -> Result<Arc<dyn HashCache>> {
        Self::create(CacheConfig::NoOp)
    }
}

/// Layered cache implementation (L1: Memory, L2: File)
struct LayeredCache {
    l1: MemoryCache,
    l2: FileCache,
}

impl LayeredCache {
    fn new(l1: MemoryCache, l2: FileCache) -> Self {
        Self { l1, l2 }
    }
}

#[async_trait::async_trait]
impl HashCache for LayeredCache {
    async fn get(
        &self,
        key: &crate::cache::CacheKey,
    ) -> Result<Option<anidb_client_core::hashing::HashResult>> {
        // Try L1 first
        if let Some(result) = self.l1.get(key).await? {
            return Ok(Some(result));
        }

        // Fall back to L2
        if let Some(result) = self.l2.get(key).await? {
            // Promote to L1 with short TTL
            let _ = self
                .l1
                .put_with_ttl(key, &result, Duration::from_secs(3600))
                .await;
            return Ok(Some(result));
        }

        Ok(None)
    }

    async fn put(
        &self,
        key: &crate::cache::CacheKey,
        value: &anidb_client_core::hashing::HashResult,
    ) -> Result<()> {
        // Write to both layers
        self.l1.put(key, value).await?;
        self.l2.put(key, value).await?;
        Ok(())
    }

    async fn put_with_ttl(
        &self,
        key: &crate::cache::CacheKey,
        value: &anidb_client_core::hashing::HashResult,
        ttl: Duration,
    ) -> Result<()> {
        // Write to both layers
        self.l1.put_with_ttl(key, value, ttl).await?;
        self.l2.put_with_ttl(key, value, ttl).await?;
        Ok(())
    }

    async fn invalidate(&self, key: &crate::cache::CacheKey) -> Result<()> {
        // Invalidate in both layers
        self.l1.invalidate(key).await?;
        self.l2.invalidate(key).await?;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        // Clear both layers
        self.l1.clear().await?;
        self.l2.clear().await?;
        Ok(())
    }

    async fn stats(&self) -> Result<crate::cache::CacheStats> {
        // Combine stats from both layers
        let l1_stats = self.l1.stats().await?;
        let l2_stats = self.l2.stats().await?;

        Ok(crate::cache::CacheStats {
            entry_count: l1_stats.entry_count + l2_stats.entry_count,
            hit_count: l1_stats.hit_count + l2_stats.hit_count,
            miss_count: l1_stats.miss_count, // Only count L1 misses
            total_size_bytes: l1_stats.total_size_bytes + l2_stats.total_size_bytes,
        })
    }
}
