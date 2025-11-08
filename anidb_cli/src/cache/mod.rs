//! Caching functionality for the AniDB CLI
//!
//! This module contains caching implementations.
//!
//! The cache module has been refactored to support multiple implementations
//! through a trait-based abstraction. The original Cache type is now an alias
//! for FileCache to maintain backward compatibility.

// The actual cache implementation is in cache_impl module
// We re-export here to maintain the original module structure

// Import what we need for type definitions
use anidb_client_core::hashing::{HashAlgorithm, HashResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Cache key for identifying entries
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    pub file_path: PathBuf,
    pub file_size: u64,
    pub algorithm: HashAlgorithm,
}

impl CacheKey {
    /// Create a new cache key
    pub fn new(file_path: &Path, file_size: u64, algorithm: HashAlgorithm) -> Self {
        Self {
            file_path: file_path.to_path_buf(),
            file_size,
            algorithm,
        }
    }
}

/// Cache entry storing hash results with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub hash_result: HashResult,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
    pub access_count: u64,
    pub expires_at: Option<SystemTime>,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub entry_count: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub total_size_bytes: u64,
}

// Re-export sub-modules
pub mod factory;
pub mod file_cache;
pub mod identification_service;
pub mod memory_cache;
pub mod noop_cache;
pub mod service;
pub mod sqlite_cache;
pub mod traits;

// Re-export commonly used types
pub use identification_service::IdentificationCacheService;
