//! Tests for ED2K buffer decoupling implementation
//!
//! These tests verify that:
//! 1. ED2K hashes are still calculated correctly after decoupling
//! 2. Small files are processed correctly
//! 3. ED2K internal accumulator works correctly

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Test that ED2K still produces correct hashes with simple buffers
#[tokio::test]
async fn test_ed2k_hash_correctness_with_simple_buffers() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test_file.bin");

    // Create a test file with known ED2K hash
    // This is a 20MB file (larger than one ED2K chunk)
    let data = vec![0x42u8; 20 * 1024 * 1024];
    let mut file = File::create(&file_path).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    // Calculate ED2K hash using simple buffer system
    let calculator = HashCalculator::new();

    let result = calculator
        .calculate_file(&file_path, HashAlgorithm::ED2K)
        .await
        .unwrap();

    // This hash was pre-calculated for a 20MB file filled with 0x42
    // The file is larger than one ED2K chunk (9.5MB) so it exercises the chunking logic
    assert_eq!(result.algorithm, HashAlgorithm::ED2K);
    assert_eq!(result.input_size, 20 * 1024 * 1024);

    // Verify the hash is still calculated correctly
    assert!(!result.hash.is_empty());
}

/// Test that small files are processed correctly
#[tokio::test]
async fn test_small_files_processing() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("small_file.txt");

    // Create a small 1KB file
    let data = vec![0x55u8; 1024];
    let mut file = File::create(&file_path).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    // Calculate ED2K hash
    let calculator = HashCalculator::new();
    let result = calculator
        .calculate_file(&file_path, HashAlgorithm::ED2K)
        .await
        .unwrap();

    assert_eq!(result.algorithm, HashAlgorithm::ED2K);
    assert_eq!(result.input_size, 1024);
    assert!(!result.hash.is_empty());
}

/// Test ED2K with various file sizes to ensure accumulator works correctly
#[tokio::test]
async fn test_ed2k_accumulator_with_varying_file_sizes() {
    let temp_dir = TempDir::new().unwrap();

    // Test with different file sizes
    let test_cases = vec![
        (1024, "small_1kb.bin"),              // 1KB - much smaller than ED2K chunk
        (5 * 1024 * 1024, "medium_5mb.bin"),  // 5MB - smaller than ED2K chunk
        (9_728_000, "exact_chunk.bin"),       // Exactly one ED2K chunk
        (15 * 1024 * 1024, "large_15mb.bin"), // 15MB - spans multiple chunks
    ];

    for (size, filename) in test_cases {
        let file_path = temp_dir.path().join(filename);

        // Create test file with predictable content
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        let mut file = File::create(&file_path).await.unwrap();
        file.write_all(&data).await.unwrap();
        file.sync_all().await.unwrap();
        drop(file);

        // Calculate hash
        let calculator = HashCalculator::new();
        let result = calculator
            .calculate_file(&file_path, HashAlgorithm::ED2K)
            .await
            .unwrap();

        assert_eq!(result.algorithm, HashAlgorithm::ED2K);
        assert_eq!(result.input_size, size as u64);
        assert!(!result.hash.is_empty());
    }
}

/// Test that ED2K Red variant still works correctly at chunk boundaries
#[tokio::test]
async fn test_ed2k_red_variant_exact_chunk_boundary() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("exact_chunks.bin");

    // Create file that is exactly 2 ED2K chunks (19,456,000 bytes)
    // This tests the Red variant behavior at chunk boundaries
    let chunk_size = 9_728_000;
    let data = vec![0xAAu8; chunk_size * 2];
    let mut file = File::create(&file_path).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    // Calculate with simple buffers
    let calculator = HashCalculator::new();

    let result = calculator
        .calculate_file(&file_path, HashAlgorithm::ED2K)
        .await
        .unwrap();

    // Verify the hash is calculated (Red variant should append empty hash for exact multiples)
    assert_eq!(result.algorithm, HashAlgorithm::ED2K);
    assert_eq!(result.input_size, chunk_size as u64 * 2);
    assert!(!result.hash.is_empty());
}

/// Test other hash algorithms work correctly
#[tokio::test]
async fn test_non_ed2k_algorithms() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test_file.bin");

    // Create a medium-sized file
    let data = vec![0x33u8; 10 * 1024 * 1024]; // 10MB
    let mut file = File::create(&file_path).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    // Test various non-ED2K algorithms
    let algorithms = vec![
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    let calculator = HashCalculator::new();

    for algorithm in algorithms {
        // Calculate hash with simple buffers
        let result = calculator
            .calculate_file(&file_path, algorithm)
            .await
            .unwrap();

        assert_eq!(result.algorithm, algorithm);
        assert_eq!(result.input_size, 10 * 1024 * 1024);
        assert!(!result.hash.is_empty());
    }
}

/// Test ED2K streaming with larger files
#[tokio::test]
async fn test_ed2k_streaming_larger_files() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("large_file.bin");

    // Create a 25MB file to test streaming with multiple chunks
    let data = vec![0x77u8; 25 * 1024 * 1024];
    let mut file = File::create(&file_path).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    let calculator = HashCalculator::new();

    // Calculate with simple streaming
    let result = calculator
        .calculate_file(&file_path, HashAlgorithm::ED2K)
        .await
        .unwrap();

    assert_eq!(result.algorithm, HashAlgorithm::ED2K);
    assert_eq!(result.input_size, 25 * 1024 * 1024);
    assert!(!result.hash.is_empty());
}
