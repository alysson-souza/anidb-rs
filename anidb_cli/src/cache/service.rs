//! Cache service wrapper for the AniDB CLI
//!
//! This module provides a service layer that adds caching functionality
//! on top of the core AniDBClient, transparently handling cache lookups
//! and storage for file processing operations.

use crate::cache::traits::HashCache;
use crate::cache::{CacheKey, CacheStats};
use anidb_client_core::{
    api::{AniDBClient, BatchOptions, BatchResult, FileResult, ProcessOptions},
    error::Result,
    hashing::HashAlgorithm,
    progress::ProgressProvider,
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Hash cache service that wraps the core AniDBClient with caching functionality
///
/// This service acts as a layer between CLI commands and the core library,
/// adding transparent caching support for file hash calculations.
pub struct HashCacheService {
    /// The underlying AniDB client from the core library
    client: Arc<AniDBClient>,
    /// The cache implementation to use
    cache: Arc<dyn HashCache>,
    /// Enable verbose logging
    verbose: bool,
}

impl HashCacheService {
    /// Create a new HashCacheService
    ///
    /// # Arguments
    ///
    /// * `client` - The AniDBClient from the core library
    /// * `cache` - The cache implementation to use
    pub fn new(client: Arc<AniDBClient>, cache: Arc<dyn HashCache>) -> Self {
        Self {
            client,
            cache,
            verbose: false,
        }
    }

    /// Create a new HashCacheService with verbose logging
    #[allow(dead_code)]
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Process a file with cache support
    ///
    /// This method checks the cache first if caching is enabled in the options,
    /// processes the file if needed, and stores the result in the cache.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to process
    /// * `options` - Processing options including algorithms and cache settings
    /// * `use_cache` - Whether to use caching for this operation
    ///
    /// # Returns
    ///
    /// A Result containing the file processing results or an error
    #[allow(dead_code)]
    pub async fn process_file_with_cache(
        &self,
        file_path: &Path,
        options: ProcessOptions,
        use_cache: bool,
    ) -> Result<FileResult> {
        // Get file metadata for cache key generation
        let metadata = tokio::fs::metadata(file_path).await.map_err(|_e| {
            anidb_client_core::Error::Io(anidb_client_core::error::IoError::file_not_found(
                file_path,
            ))
        })?;
        let file_size = metadata.len();

        // Check cache for each requested algorithm if caching is enabled
        if use_cache {
            let mut cached_hashes = HashMap::new();
            let mut missing_algorithms = Vec::new();

            for algorithm in options.algorithms() {
                let cache_key = CacheKey::new(file_path, file_size, *algorithm);

                if let Ok(Some(hash_result)) = self.cache.get(&cache_key).await {
                    if self.verbose {
                        log::debug!("Cache hit for {file_path:?} with algorithm {algorithm:?}");
                    }
                    cached_hashes.insert(*algorithm, hash_result.hash);
                } else {
                    missing_algorithms.push(*algorithm);
                }
            }

            // If all algorithms are cached, return early
            if missing_algorithms.is_empty() && !cached_hashes.is_empty() {
                if self.verbose {
                    log::debug!("All algorithms found in cache for {file_path:?}");
                }

                return Ok(FileResult {
                    file_path: file_path.to_path_buf(),
                    file_size,
                    hashes: cached_hashes,
                    status: anidb_client_core::file_io::ProcessingStatus::Completed,
                    processing_time: Duration::from_millis(0), // Cached results have no processing time
                    anime_info: None,
                });
            }

            // Process only missing algorithms if some were cached
            if !cached_hashes.is_empty() {
                let partial_options = options.clone().with_algorithms(&missing_algorithms);
                let result = self.client.process_file(file_path, partial_options).await?;

                // Store newly calculated hashes in cache
                for (algorithm, hash) in &result.hashes {
                    let cache_key = CacheKey::new(file_path, file_size, *algorithm);
                    let hash_result = anidb_client_core::hashing::HashResult {
                        hash: hash.clone(),
                        algorithm: *algorithm,
                        input_size: file_size,
                        duration: result.processing_time,
                    };

                    if let Err(e) = self.cache.put(&cache_key, &hash_result).await
                        && self.verbose
                    {
                        log::warn!("Failed to cache result for {cache_key:?}: {e}");
                    }
                }

                // Merge cached and newly calculated hashes
                let mut all_hashes = cached_hashes;
                all_hashes.extend(result.hashes);

                return Ok(FileResult {
                    file_path: result.file_path,
                    file_size: result.file_size,
                    hashes: all_hashes,
                    status: result.status,
                    processing_time: result.processing_time,
                    anime_info: result.anime_info,
                });
            }
        }

        // No cache or cache miss for all algorithms - process normally
        let result = self.client.process_file(file_path, options).await?;

        // Store result in cache if caching is enabled
        if use_cache {
            for (algorithm, hash) in &result.hashes {
                let cache_key = CacheKey::new(file_path, file_size, *algorithm);
                let hash_result = anidb_client_core::hashing::HashResult {
                    hash: hash.clone(),
                    algorithm: *algorithm,
                    input_size: file_size,
                    duration: result.processing_time,
                };

                if let Err(e) = self.cache.put(&cache_key, &hash_result).await
                    && self.verbose
                {
                    log::warn!("Failed to cache result for {cache_key:?}: {e}");
                }
            }
        }

        Ok(result)
    }

    /// Process a file with cache support and progress reporting
    ///
    /// Similar to `process_file_with_cache` but with progress reporting support.
    pub async fn process_file_with_cache_and_progress(
        &self,
        file_path: &Path,
        options: ProcessOptions,
        use_cache: bool,
        progress: Arc<dyn ProgressProvider>,
    ) -> Result<FileResult> {
        // Get file metadata for cache key generation
        let metadata = tokio::fs::metadata(file_path).await.map_err(|_e| {
            anidb_client_core::Error::Io(anidb_client_core::error::IoError::file_not_found(
                file_path,
            ))
        })?;
        let file_size = metadata.len();

        // Check cache for each requested algorithm if caching is enabled
        if use_cache {
            let mut cached_hashes = HashMap::new();
            let mut missing_algorithms = Vec::new();

            for algorithm in options.algorithms() {
                let cache_key = CacheKey::new(file_path, file_size, *algorithm);

                if let Ok(Some(hash_result)) = self.cache.get(&cache_key).await {
                    if self.verbose {
                        log::debug!("Cache hit for {file_path:?} with algorithm {algorithm:?}");
                    }
                    cached_hashes.insert(*algorithm, hash_result.hash);
                } else {
                    missing_algorithms.push(*algorithm);
                }
            }

            // If all algorithms are cached, return early
            if missing_algorithms.is_empty() && !cached_hashes.is_empty() {
                if self.verbose {
                    log::debug!("All algorithms found in cache for {file_path:?}");
                }

                return Ok(FileResult {
                    file_path: file_path.to_path_buf(),
                    file_size,
                    hashes: cached_hashes,
                    status: anidb_client_core::file_io::ProcessingStatus::Completed,
                    processing_time: Duration::from_millis(0),
                    anime_info: None,
                });
            }

            // Process only missing algorithms if some were cached
            if !cached_hashes.is_empty() {
                let partial_options = options.clone().with_algorithms(&missing_algorithms);
                let result = self
                    .client
                    .process_file_with_progress(file_path, partial_options, progress)
                    .await?;

                // Store newly calculated hashes in cache
                for (algorithm, hash) in &result.hashes {
                    let cache_key = CacheKey::new(file_path, file_size, *algorithm);
                    let hash_result = anidb_client_core::hashing::HashResult {
                        hash: hash.clone(),
                        algorithm: *algorithm,
                        input_size: file_size,
                        duration: result.processing_time,
                    };

                    if let Err(e) = self.cache.put(&cache_key, &hash_result).await
                        && self.verbose
                    {
                        log::warn!("Failed to cache result for {cache_key:?}: {e}");
                    }
                }

                // Merge cached and newly calculated hashes
                let mut all_hashes = cached_hashes;
                all_hashes.extend(result.hashes);

                return Ok(FileResult {
                    file_path: result.file_path,
                    file_size: result.file_size,
                    hashes: all_hashes,
                    status: result.status,
                    processing_time: result.processing_time,
                    anime_info: result.anime_info,
                });
            }
        }

        // No cache or cache miss for all algorithms - process normally
        let result = self
            .client
            .process_file_with_progress(file_path, options, progress)
            .await?;

        // Store result in cache if caching is enabled
        if use_cache {
            for (algorithm, hash) in &result.hashes {
                let cache_key = CacheKey::new(file_path, file_size, *algorithm);
                let hash_result = anidb_client_core::hashing::HashResult {
                    hash: hash.clone(),
                    algorithm: *algorithm,
                    input_size: file_size,
                    duration: result.processing_time,
                };

                if let Err(e) = self.cache.put(&cache_key, &hash_result).await
                    && self.verbose
                {
                    log::warn!("Failed to cache result for {cache_key:?}: {e}");
                }
            }
        }

        Ok(result)
    }

    /// Process multiple files in batch with cache support
    ///
    /// This method processes multiple files, checking the cache for each file
    /// and only processing files that aren't fully cached.
    ///
    /// # Arguments
    ///
    /// * `file_paths` - Slice of file paths to process
    /// * `options` - Batch processing options
    /// * `use_cache` - Whether to use caching for this operation
    ///
    /// # Returns
    ///
    /// A Result containing the batch processing results or an error
    #[allow(dead_code)]
    pub async fn process_batch_with_cache(
        &self,
        file_paths: &[std::path::PathBuf],
        options: BatchOptions,
        use_cache: bool,
    ) -> Result<BatchResult> {
        let start = std::time::Instant::now();
        let mut results = Vec::new();

        // Process each file with caching support
        for file_path in file_paths {
            // Create process options from batch options
            let process_options = ProcessOptions::new().with_algorithms(options.algorithms());

            let result = self
                .process_file_with_cache(file_path, process_options, use_cache)
                .await;

            results.push(result);
        }

        // Calculate statistics
        let total_files = results.len();
        let successful_files = results.iter().filter(|r| r.is_ok()).count();
        let failed_files = total_files - successful_files;

        Ok(BatchResult {
            total_files,
            successful_files,
            failed_files,
            results,
            total_time: start.elapsed(),
        })
    }

    /// Get cache statistics
    ///
    /// Returns statistics about cache usage including hit/miss rates and entry counts.
    #[allow(dead_code)]
    pub async fn get_cache_stats(&self) -> Result<CacheStats> {
        self.cache.stats().await
    }

    /// Clear the cache
    ///
    /// Removes all entries from the cache.
    #[allow(dead_code)]
    pub async fn clear_cache(&self) -> Result<()> {
        self.cache.clear().await
    }

    /// Invalidate a specific cache entry
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file
    /// * `file_size` - Size of the file in bytes
    /// * `algorithm` - Hash algorithm to invalidate
    #[allow(dead_code)]
    pub async fn invalidate_cache_entry(
        &self,
        file_path: &Path,
        file_size: u64,
        algorithm: HashAlgorithm,
    ) -> Result<()> {
        let cache_key = CacheKey::new(file_path, file_size, algorithm);
        self.cache.invalidate(&cache_key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::memory_cache::MemoryCache;
    use anidb_client_core::{ClientConfig, HashAlgorithm};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_service_creation() {
        let config = ClientConfig::test();

        let client = Arc::new(AniDBClient::new(config).await.unwrap());
        let cache = Arc::new(MemoryCache::new());

        let service = HashCacheService::new(client, cache);
        assert!(!service.verbose);
    }

    #[tokio::test]
    async fn test_service_with_verbose() {
        let config = ClientConfig::test();

        let client = Arc::new(AniDBClient::new(config).await.unwrap());
        let cache = Arc::new(MemoryCache::new());

        let service = HashCacheService::new(client, cache).with_verbose(true);
        assert!(service.verbose);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let config = ClientConfig::test();

        let client = Arc::new(AniDBClient::new(config).await.unwrap());
        let cache = Arc::new(MemoryCache::new());
        let service = HashCacheService::new(client, cache);

        // Test cache stats
        let stats = service.get_cache_stats().await.unwrap();
        assert_eq!(stats.entry_count, 0);

        // Test clear cache
        service.clear_cache().await.unwrap();

        // Test invalidate
        let path = PathBuf::from("test.mkv");
        service
            .invalidate_cache_entry(&path, 1000, HashAlgorithm::ED2K)
            .await
            .unwrap();
    }
}
