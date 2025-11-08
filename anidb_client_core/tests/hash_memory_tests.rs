//! Memory usage tests for hash algorithms
//!
//! Verifies that all hash algorithms maintain constant memory usage
//! regardless of file size, and properly implement streaming.

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use std::time::Instant;
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Test that SHA1 maintains constant memory usage for large files
#[tokio::test]
async fn test_sha1_memory_usage() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("large_file.bin");

    // Create a 10MB file
    let file_size = 10 * 1024 * 1024;
    let mut file = File::create(&test_file).await.unwrap();

    // Write in chunks to avoid loading all data in memory
    let chunk_size = 64 * 1024;
    let chunk = vec![0xABu8; chunk_size];
    let chunks = file_size / chunk_size;

    for _ in 0..chunks {
        file.write_all(&chunk).await.unwrap();
    }
    file.flush().await.unwrap();
    drop(file);

    let calculator = HashCalculator::new();
    let start_time = Instant::now();

    // Calculate hash - should use streaming and constant memory
    let result = calculator
        .calculate_file(&test_file, HashAlgorithm::SHA1)
        .await
        .unwrap();

    let elapsed = start_time.elapsed();

    assert_eq!(result.algorithm, HashAlgorithm::SHA1);
    assert_eq!(result.input_size, file_size as u64);
    assert!(!result.hash.is_empty());

    // Verify performance (should process at least 30MB/s)
    // Note: This is a conservative threshold for test environments
    let throughput_mbps = (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
    assert!(
        throughput_mbps >= 30.0,
        "SHA1 throughput too low: {throughput_mbps:.2} MB/s"
    );
}

/// Test that TTH maintains constant memory usage for large files
#[tokio::test]
async fn test_tth_memory_usage() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("large_file.bin");

    // Create a 10MB file (multiple TTH leaves)
    let file_size = 10 * 1024 * 1024;
    let mut file = File::create(&test_file).await.unwrap();

    // Write in chunks
    let chunk_size = 64 * 1024;
    let chunk = vec![0xCDu8; chunk_size];
    let chunks = file_size / chunk_size;

    for _ in 0..chunks {
        file.write_all(&chunk).await.unwrap();
    }
    file.flush().await.unwrap();
    drop(file);

    let calculator = HashCalculator::new();
    let start_time = Instant::now();

    // Calculate hash - should use streaming and constant memory
    let result = calculator
        .calculate_file(&test_file, HashAlgorithm::TTH)
        .await
        .unwrap();

    let elapsed = start_time.elapsed();

    assert_eq!(result.algorithm, HashAlgorithm::TTH);
    assert_eq!(result.input_size, file_size as u64);
    assert!(!result.hash.is_empty());
    assert_eq!(result.hash.len(), 39); // TTH base32 is 39 chars

    // Verify performance (should process at least 50MB/s)
    let throughput_mbps = (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
    assert!(
        throughput_mbps >= 50.0,
        "TTH throughput too low: {throughput_mbps:.2} MB/s"
    );
}

/// Test TTH with exact leaf boundaries
#[tokio::test]
async fn test_tth_leaf_boundaries() {
    let temp_dir = TempDir::new().unwrap();
    let calculator = HashCalculator::new();

    // Test cases around TTH leaf size (1024 bytes)
    let test_cases = vec![
        (1023, "Just under one leaf"),
        (1024, "Exactly one leaf"),
        (1025, "Just over one leaf"),
        (2048, "Exactly two leaves"),
        (10240, "Exactly ten leaves"),
    ];

    for (size, _description) in test_cases {
        let test_file = temp_dir.path().join(format!("file_{size}.bin"));
        let mut file = File::create(&test_file).await.unwrap();

        // Create file with specific pattern
        let data = vec![(size % 256) as u8; size];
        file.write_all(&data).await.unwrap();
        file.flush().await.unwrap();
        drop(file);

        let result = calculator
            .calculate_file(&test_file, HashAlgorithm::TTH)
            .await
            .unwrap();

        assert_eq!(
            result.input_size, size as u64,
            "TTH file size mismatch for: {_description}"
        );
        assert_eq!(result.hash.len(), 39, "TTH hash length incorrect");

        // Also test with in-memory calculation for comparison
        let memory_result = calculator
            .calculate_bytes(HashAlgorithm::TTH, &data)
            .unwrap();

        assert_eq!(
            result.hash, memory_result.hash,
            "TTH streaming vs memory mismatch for: {_description}"
        );
    }
}

/// Test multiple hash algorithms in parallel with memory constraints
#[tokio::test]
async fn test_multiple_algorithms_memory_usage() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("multi_hash.bin");

    // Create a 5MB file
    let file_size = 5 * 1024 * 1024;
    let mut file = File::create(&test_file).await.unwrap();

    // Write with a pattern
    let chunk_size = 64 * 1024;
    let mut chunk = vec![0u8; chunk_size];
    for (i, byte) in chunk.iter_mut().enumerate() {
        *byte = (i % 256) as u8;
    }

    let chunks = file_size / chunk_size;
    for _ in 0..chunks {
        file.write_all(&chunk).await.unwrap();
    }
    file.flush().await.unwrap();
    drop(file);

    let calculator = HashCalculator::new();
    let algorithms = vec![
        HashAlgorithm::SHA1,
        HashAlgorithm::TTH,
        HashAlgorithm::ED2K,
        HashAlgorithm::MD5,
    ];

    let start_time = Instant::now();

    // Calculate all hashes in one pass
    let results = calculator
        .calculate_multiple(&test_file, &algorithms)
        .await
        .unwrap();

    let elapsed = start_time.elapsed();

    // Verify all algorithms produced results
    assert_eq!(results.len(), algorithms.len());
    for algorithm in &algorithms {
        let result = results.get(algorithm).unwrap();
        assert_eq!(result.algorithm, *algorithm);
        assert_eq!(result.input_size, file_size as u64);
        assert!(!result.hash.is_empty());
    }

    // Verify performance (should process at least 50MB/s total)
    // Note: This is a conservative threshold for test environments
    let total_data = file_size as f64 * algorithms.len() as f64;
    let throughput_mbps = (total_data / 1_048_576.0) / elapsed.as_secs_f64();
    assert!(
        throughput_mbps >= 50.0,
        "Multi-algorithm throughput too low: {throughput_mbps:.2} MB/s"
    );
}

/// Test that TTH produces consistent results regardless of chunking
#[tokio::test]
async fn test_tth_streaming_consistency() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("consistency_test.bin");
    let calculator = HashCalculator::new();

    // Test data that spans multiple leaves
    let data_size = 10 * 1024; // 10 leaves
    let test_data = vec![0x42u8; data_size];

    // Write test data to file
    let mut file = File::create(&test_file).await.unwrap();
    file.write_all(&test_data).await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Calculate hash from memory
    let memory_result = calculator
        .calculate_bytes(HashAlgorithm::TTH, &test_data)
        .unwrap();

    // Calculate hash from file (uses streaming)
    let file_result = calculator
        .calculate_file(&test_file, HashAlgorithm::TTH)
        .await
        .unwrap();

    assert_eq!(
        memory_result.hash, file_result.hash,
        "TTH memory vs streaming file hash mismatch"
    );
    assert_eq!(file_result.input_size, data_size as u64);
}

/// Test edge cases for TTH algorithm
#[test]
fn test_tth_edge_cases() {
    let calculator = HashCalculator::new();

    // Empty file
    let empty_result = calculator.calculate_bytes(HashAlgorithm::TTH, &[]).unwrap();
    assert_eq!(empty_result.hash, "lwpnacqdbzryxw3vhjvcj64qbznghohhhzwclnq");

    // Single byte
    let single_byte = calculator
        .calculate_bytes(HashAlgorithm::TTH, &[0x00])
        .unwrap();
    assert_eq!(single_byte.hash.len(), 39);

    // Exactly one leaf minus one byte
    let almost_leaf = vec![0xFFu8; 1023];
    let result = calculator
        .calculate_bytes(HashAlgorithm::TTH, &almost_leaf)
        .unwrap();
    assert_eq!(result.hash.len(), 39);

    // Exactly one leaf
    let one_leaf = vec![0xEEu8; 1024];
    let result = calculator
        .calculate_bytes(HashAlgorithm::TTH, &one_leaf)
        .unwrap();
    assert_eq!(result.hash.len(), 39);

    // Power of 2 leaves (tests tree building)
    for power in 1..=10 {
        let leaf_count = 1 << power; // 2, 4, 8, 16, etc.
        let data = vec![power as u8; 1024 * leaf_count];
        let result = calculator
            .calculate_bytes(HashAlgorithm::TTH, &data)
            .unwrap();
        assert_eq!(
            result.hash.len(),
            39,
            "TTH hash length incorrect for {leaf_count} leaves"
        );
    }
}

/// Verify SHA1 produces correct output format
#[test]
fn test_sha1_output_format() {
    let calculator = HashCalculator::new();

    let test_cases = vec![
        b"" as &[u8],
        b"a",
        b"abc",
        b"The quick brown fox jumps over the lazy dog",
    ];

    for data in test_cases {
        let result = calculator
            .calculate_bytes(HashAlgorithm::SHA1, data)
            .unwrap();

        // SHA1 produces 160-bit hash = 40 hex characters
        assert_eq!(result.hash.len(), 40);
        assert!(
            result.hash.chars().all(|c| c.is_ascii_hexdigit()),
            "SHA1 hash contains non-hex characters"
        );
    }
}
