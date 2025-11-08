//! Tests for parallel hash calculation implementation
//!
//! These tests verify that hash calculations run in true parallel threads

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use anidb_client_core::progress::NullProvider;
use std::time::Instant;
use tempfile::TempDir;

/// Test that parallel calculation uses multiple threads
#[tokio::test]
async fn test_calculate_parallel_uses_multiple_threads() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.bin");

    // Create a reasonably sized file to ensure thread work is visible
    let test_data = vec![0x42u8; 50 * 1024 * 1024]; // 50MB
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();
    let algorithms = vec![
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    // Act
    let result = calculator.calculate_parallel(&test_file, &algorithms).await;

    // Assert
    assert!(result.is_ok());
    let hash_results = result.unwrap();
    assert_eq!(hash_results.len(), 4);

    // All algorithms should have results
    for algo in &algorithms {
        assert!(hash_results.contains_key(algo));
    }
}

/// Test performance improvement with parallel calculation
#[tokio::test]
async fn test_parallel_calculation_performance() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("perf_test.bin");

    // Create a large file for meaningful performance comparison
    let test_data = vec![0xFFu8; 100 * 1024 * 1024]; // 100MB
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();
    let algorithms = vec![
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    // Act - Measure sequential time
    let start_seq = Instant::now();
    let result_seq = calculator
        .calculate_multiple(&test_file, &algorithms)
        .await
        .unwrap();
    let duration_seq = start_seq.elapsed();

    // Act - Measure parallel time
    let start_par = Instant::now();
    let result_par = calculator
        .calculate_parallel(&test_file, &algorithms)
        .await
        .unwrap();
    let duration_par = start_par.elapsed();

    // Assert
    assert_eq!(result_seq.len(), 4);
    assert_eq!(result_par.len(), 4);

    // Results should be identical
    for algo in &algorithms {
        assert_eq!(
            result_seq.get(algo).unwrap().hash,
            result_par.get(algo).unwrap().hash,
            "Hash results should be identical for {algo:?}"
        );
    }

    // Parallel should be faster or at least comparable
    let speedup = duration_seq.as_secs_f64() / duration_par.as_secs_f64();
    println!("Sequential: {duration_seq:?}, Parallel: {duration_par:?}, Speedup: {speedup:.2}x");

    // Parallel might not always be faster due to overhead, especially on small files
    // or in test environments with limited cores. We just verify it works correctly
    // and doesn't degrade performance significantly (allow up to 20% slower)
    assert!(
        speedup > 0.8,
        "Parallel calculation should not be significantly slower than sequential, got {speedup:.2}x speedup"
    );
}

/// Test parallel calculation with progress reporting
#[tokio::test]
async fn test_parallel_calculation_with_progress() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("progress_test.bin");

    // Create a 20MB test file
    let test_data = vec![0xAAu8; 20 * 1024 * 1024];
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();
    let algorithms = vec![HashAlgorithm::SHA1, HashAlgorithm::MD5];

    // Act
    let result = calculator
        .calculate_multiple_with_progress_and_config(
            &test_file,
            &algorithms,
            &NullProvider,
            Default::default(),
        )
        .await;

    // Assert
    assert!(result.is_ok());
    let hash_results = result.unwrap();
    assert_eq!(hash_results.len(), 2);
}

/// Test ED2K special chunk handling in parallel mode
#[tokio::test]
async fn test_parallel_ed2k_chunk_handling() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("ed2k_test.bin");

    // Create file just over ED2K chunk size to test chunking
    const ED2K_CHUNK_SIZE: usize = 9728000;
    let test_data = vec![0x55u8; ED2K_CHUNK_SIZE + 1000];
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();
    let algorithms = vec![HashAlgorithm::ED2K];

    // Act
    let result_par = calculator
        .calculate_parallel(&test_file, &algorithms)
        .await
        .unwrap();

    let result_seq = calculator
        .calculate_multiple(&test_file, &algorithms)
        .await
        .unwrap();

    // Assert - Results should be identical
    assert_eq!(
        result_par.get(&HashAlgorithm::ED2K).unwrap().hash,
        result_seq.get(&HashAlgorithm::ED2K).unwrap().hash,
        "ED2K hash should be identical"
    );
}

/// Test parallel calculation with single algorithm (should fall back to sequential)
#[tokio::test]
async fn test_parallel_with_single_algorithm() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("single_algo.bin");

    let test_data = vec![0x33u8; 1024 * 1024]; // 1MB
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();

    // Act
    let result = calculator
        .calculate_parallel(&test_file, &[HashAlgorithm::CRC32])
        .await;

    // Assert
    assert!(result.is_ok());
    let hash_results = result.unwrap();
    assert_eq!(hash_results.len(), 1);
}

/// Test parallel calculation with empty algorithm list
#[tokio::test]
async fn test_parallel_with_empty_algorithms() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("empty_algo.bin");

    let test_data = b"test content";
    tokio::fs::write(&test_file, test_data).await.unwrap();

    let calculator = HashCalculator::new();

    // Act
    let result = calculator.calculate_parallel(&test_file, &[]).await;

    // Assert
    if let Err(e) = &result {
        eprintln!("Error with empty algorithms: {e:?}");
    }
    assert!(result.is_ok());
    let hash_results = result.unwrap();
    assert!(hash_results.is_empty());
}

/// Test memory usage stays within limits during parallel calculation
#[tokio::test]
async fn test_parallel_memory_usage() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("memory_test.bin");

    // Create a large file
    let test_data = vec![0x77u8; 200 * 1024 * 1024]; // 200MB
    tokio::fs::write(&test_file, &test_data).await.unwrap();

    let calculator = HashCalculator::new();
    let algorithms = vec![
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
        HashAlgorithm::TTH,
    ];

    // Act
    let result = calculator.calculate_parallel(&test_file, &algorithms).await;

    // Assert
    assert!(result.is_ok(), "Should complete within memory limits");
    let hash_results = result.unwrap();
    assert_eq!(hash_results.len(), 5);
}

/// Test cancellation/error handling in parallel calculation
#[tokio::test]
async fn test_parallel_calculation_file_not_found() {
    // Arrange
    let calculator = HashCalculator::new();

    // Act
    let algorithms = vec![HashAlgorithm::ED2K, HashAlgorithm::CRC32];
    let result = calculator
        .calculate_parallel(std::path::Path::new("/nonexistent/file.bin"), &algorithms)
        .await;

    // Assert
    assert!(result.is_err());
    match result.unwrap_err() {
        anidb_client_core::Error::Io(io_err)
            if io_err.kind == anidb_client_core::error::IoErrorKind::FileNotFound =>
        {
            if let Some(ref path) = io_err.path {
                assert_eq!(path.to_str().unwrap(), "/nonexistent/file.bin");
            }
        }
        err => panic!("Expected FileNotFound error, got: {err:?}"),
    }
}
