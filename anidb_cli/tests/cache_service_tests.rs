//! Integration tests for the cache service
//!
//! These tests verify that the cache service correctly integrates
//! with the core AniDBClient and cache implementations.

use anidb_cli::cache::{memory_cache::MemoryCache, service::HashCacheService};
use anidb_client_core::{
    ClientConfig, HashAlgorithm,
    api::{AniDBClient, ProcessOptions},
};
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_cache_service_integration() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, b"test content").unwrap();

    // Setup client and cache
    let config = ClientConfig::test();

    let client = Arc::new(AniDBClient::new(config).await.unwrap());
    let cache = Arc::new(MemoryCache::new());
    let service = HashCacheService::new(client, cache.clone()).with_verbose(true);

    // Create process options
    let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::MD5, HashAlgorithm::SHA1]);

    // First call - should calculate hashes
    let result1 = service
        .process_file_with_cache(&test_file, options.clone(), true)
        .await
        .unwrap();

    assert_eq!(result1.hashes.len(), 2);
    assert!(result1.hashes.contains_key(&HashAlgorithm::MD5));
    assert!(result1.hashes.contains_key(&HashAlgorithm::SHA1));
    assert!(result1.processing_time.as_millis() > 0);

    // Second call - should use cache
    let result2 = service
        .process_file_with_cache(&test_file, options, true)
        .await
        .unwrap();

    assert_eq!(result2.hashes.len(), 2);
    assert_eq!(
        result1.hashes[&HashAlgorithm::MD5],
        result2.hashes[&HashAlgorithm::MD5]
    );
    assert_eq!(
        result1.hashes[&HashAlgorithm::SHA1],
        result2.hashes[&HashAlgorithm::SHA1]
    );
    // Cached results should have 0 processing time
    assert_eq!(result2.processing_time.as_millis(), 0);

    // Check cache stats
    let stats = service.get_cache_stats().await.unwrap();
    assert_eq!(stats.entry_count, 2); // MD5 and SHA1
}

#[tokio::test]
async fn test_cache_service_no_cache() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, b"test content").unwrap();

    // Setup client and cache
    let config = ClientConfig::test();

    let client = Arc::new(AniDBClient::new(config).await.unwrap());
    let cache = Arc::new(MemoryCache::new());
    let service = HashCacheService::new(client, cache.clone());

    // Create process options
    let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::CRC32]);

    // Process without caching
    let result = service
        .process_file_with_cache(&test_file, options, false)
        .await
        .unwrap();

    assert_eq!(result.hashes.len(), 1);
    assert!(result.hashes.contains_key(&HashAlgorithm::CRC32));

    // Check cache is empty
    let stats = service.get_cache_stats().await.unwrap();
    assert_eq!(stats.entry_count, 0);
}

#[tokio::test]
async fn test_cache_service_partial_cache() {
    // Create temporary directory for test
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, b"test content for partial cache").unwrap();

    // Setup client and cache
    let config = ClientConfig::test();

    let client = Arc::new(AniDBClient::new(config).await.unwrap());
    let cache = Arc::new(MemoryCache::new());
    let service = HashCacheService::new(client, cache.clone());

    // First call with one algorithm
    let options1 = ProcessOptions::new().with_algorithms(&[HashAlgorithm::MD5]);

    let _result1 = service
        .process_file_with_cache(&test_file, options1, true)
        .await
        .unwrap();

    // Second call with two algorithms (one cached, one new)
    let options2 =
        ProcessOptions::new().with_algorithms(&[HashAlgorithm::MD5, HashAlgorithm::SHA1]);

    let result2 = service
        .process_file_with_cache(&test_file, options2, true)
        .await
        .unwrap();

    assert_eq!(result2.hashes.len(), 2);
    assert!(result2.hashes.contains_key(&HashAlgorithm::MD5));
    assert!(result2.hashes.contains_key(&HashAlgorithm::SHA1));

    // Cache should have both algorithms now
    let stats = service.get_cache_stats().await.unwrap();
    assert_eq!(stats.entry_count, 2);
}
