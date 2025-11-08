//! No-operation cache implementation
//!
//! This module provides a cache implementation that doesn't store anything,
//! useful for testing or when caching is disabled.

use crate::cache::traits::HashCache;
use crate::cache::{CacheKey, CacheStats};
use anidb_client_core::error::Result;
use anidb_client_core::hashing::HashResult;
use async_trait::async_trait;
use std::time::Duration;

/// A cache implementation that doesn't cache anything
///
/// This is useful for:
/// - Testing without cache interference
/// - Disabling caching in certain environments
/// - Benchmarking without cache effects
pub struct NoOpCache;

impl NoOpCache {
    /// Create a new no-op cache
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl HashCache for NoOpCache {
    async fn get(&self, _key: &CacheKey) -> Result<Option<HashResult>> {
        // Always return None (cache miss)
        Ok(None)
    }

    async fn put(&self, _key: &CacheKey, _value: &HashResult) -> Result<()> {
        // Silently discard the value
        Ok(())
    }

    async fn put_with_ttl(
        &self,
        _key: &CacheKey,
        _value: &HashResult,
        _ttl: Duration,
    ) -> Result<()> {
        // Silently discard the value
        Ok(())
    }

    async fn invalidate(&self, _key: &CacheKey) -> Result<()> {
        // Nothing to invalidate
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        // Nothing to clear
        Ok(())
    }

    async fn stats(&self) -> Result<CacheStats> {
        // Return empty stats
        Ok(CacheStats::default())
    }
}

impl Default for NoOpCache {
    fn default() -> Self {
        Self::new()
    }
}
