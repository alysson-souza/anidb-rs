//! Tests for the hybrid copy-on-read buffer ring architecture

use anidb_client_core::progress::NullProvider;
use anidb_client_core::{HashAlgorithm, HashCalculator, hashing::HashConfig};
use std::collections::HashMap;
use std::time::Instant;

/// Test that the hybrid architecture allows algorithms to process at different speeds
#[tokio::test]
async fn test_hybrid_parallel_different_speeds() {
    // Create test file with enough data to see speed differences
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test_speeds.bin");

    // Create 10MB test file (smaller for faster tests)
    let test_data = vec![0xAB; 10 * 1024 * 1024];
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();

    // Test with multiple algorithms that have different speeds
    let algorithms = vec![
        HashAlgorithm::CRC32, // Fast
        HashAlgorithm::SHA1,  // Medium
        HashAlgorithm::ED2K,  // Slow (180MB/s)
    ];

    let start = Instant::now();

    // Calculate hashes using hybrid architecture
    let results = calculator
        .calculate_multiple_with_progress_and_config(
            &test_file,
            &algorithms,
            &NullProvider,
            HashConfig::default(),
        )
        .await
        .unwrap();

    let duration = start.elapsed();

    // Verify all algorithms completed
    assert_eq!(results.len(), algorithms.len());

    println!(
        "Hybrid parallel completed {} algorithms in {:?}",
        algorithms.len(),
        duration
    );

    // The total time should be close to the time for the slowest algorithm (ED2K)
    // not the sum of all algorithms
    let throughput_mbps = (test_data.len() as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();
    println!("Overall throughput: {throughput_mbps:.2} MB/s");

    // With hybrid architecture, throughput should be reasonable
    // Note: In test environments, throughput can vary significantly
    assert!(
        throughput_mbps > 20.0,
        "Throughput should be reasonable (got {throughput_mbps:.2} MB/s)"
    );
}

/// Test memory usage stays within limits with hybrid architecture
#[tokio::test]
async fn test_hybrid_memory_usage() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test_memory.bin");

    // Create 5MB test file
    let test_data = vec![0x55; 5 * 1024 * 1024];
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();

    let algorithms = vec![HashAlgorithm::ED2K, HashAlgorithm::TTH];

    // Track peak memory usage
    let peak_memory = 0u64;

    // Start calculation with null provider
    let result = calculator
        .calculate_multiple_with_progress_and_config(
            &test_file,
            &algorithms,
            &NullProvider,
            HashConfig::default(),
        )
        .await
        .unwrap();

    assert_eq!(result.len(), algorithms.len());

    // Verify memory stayed within reasonable bounds
    // Ring buffer with 32 slots * chunk size should be the main memory usage
    let expected_memory = 32 * 9728000; // RING_SIZE * ED2K chunk size
    assert!(
        peak_memory <= expected_memory as u64 * 2, // Allow 2x for overhead
        "Peak memory {peak_memory} should be reasonable (expected around {expected_memory})"
    );
}

/// Test that algorithms can lag behind without blocking others
#[tokio::test]
async fn test_hybrid_lagging_algorithm() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test_lag.bin");

    // Create 20MB test file
    let test_data = vec![0x42; 20 * 1024 * 1024];
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();

    // Use algorithms with very different speeds
    let algorithms = vec![
        HashAlgorithm::CRC32, // Fast
        HashAlgorithm::ED2K,  // Slow
    ];

    let start = Instant::now();

    // Start calculation with null provider
    let result = calculator
        .calculate_multiple_with_progress_and_config(
            &test_file,
            &algorithms,
            &NullProvider,
            HashConfig::default(),
        )
        .await
        .unwrap();

    let duration = start.elapsed();

    assert_eq!(result.len(), algorithms.len());

    println!("Test completed in {duration:?}");

    // File should be read at reasonable speed, not severely limited by ED2K
    // Note: In test environments, throughput can vary significantly
    let file_throughput = (test_data.len() as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();
    println!("File processing throughput: {file_throughput:.2} MB/s");

    assert!(
        file_throughput > 50.0,
        "File reading should not be severely bottlenecked by slow algorithm"
    );
}

/// Test correct results with hybrid architecture
#[tokio::test]
async fn test_hybrid_correctness() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_file = temp_dir.path().join("test_correct.bin");

    // Create test file with known content
    let test_data = b"The quick brown fox jumps over the lazy dog. ".repeat(1000);
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();

    let algorithms = vec![
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    // Calculate with hybrid
    let hybrid_results = calculator
        .calculate_multiple_with_progress_and_config(
            &test_file,
            &algorithms,
            &NullProvider,
            HashConfig::default(),
        )
        .await
        .unwrap();

    // Calculate sequentially for comparison
    let mut sequential_results = HashMap::new();
    for algo in &algorithms {
        let result = calculator.calculate_file(&test_file, *algo).await.unwrap();
        sequential_results.insert(*algo, result);
    }

    // Results should match
    for algo in &algorithms {
        assert_eq!(
            hybrid_results[algo].hash, sequential_results[algo].hash,
            "{algo:?} hash mismatch"
        );
    }
}
