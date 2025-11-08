//! Cache trait definitions
//!
//! This module defines the core HashCache trait that all cache implementations must implement.

use crate::cache::{CacheKey, CacheStats};
use anidb_client_core::error::Result;
use anidb_client_core::hashing::HashResult;
use async_trait::async_trait;
use std::time::Duration;

/// Trait for hash cache implementations
#[async_trait]
pub trait HashCache: Send + Sync {
    /// Get a hash result from the cache
    ///
    /// Returns `Ok(Some(HashResult))` if the entry exists and is valid,
    /// `Ok(None)` if the entry doesn't exist or has expired,
    /// or an error if the operation failed.
    async fn get(&self, key: &CacheKey) -> Result<Option<HashResult>>;

    /// Store a hash result in the cache with default TTL
    ///
    /// The default TTL is implementation-specific (typically 30 days for file cache).
    async fn put(&self, key: &CacheKey, value: &HashResult) -> Result<()>;

    /// Store a hash result with custom TTL
    ///
    /// The entry will expire after the specified duration.
    async fn put_with_ttl(&self, key: &CacheKey, value: &HashResult, ttl: Duration) -> Result<()>;

    /// Invalidate a specific cache entry
    ///
    /// Removes the entry from the cache if it exists.
    #[allow(dead_code)]
    async fn invalidate(&self, key: &CacheKey) -> Result<()>;

    /// Clear all cache entries
    ///
    /// Removes all entries from the cache.
    #[allow(dead_code)]
    async fn clear(&self) -> Result<()>;

    /// Get cache statistics
    ///
    /// Returns statistics about cache usage including hit/miss rates and entry counts.
    #[allow(dead_code)]
    async fn stats(&self) -> Result<CacheStats>;
}
