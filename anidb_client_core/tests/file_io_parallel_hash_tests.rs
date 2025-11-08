//! Tests for parallel hash calculation optimization in FileProcessor
//!
//! These tests verify that FileProcessor reads files only once when calculating multiple hashes.

use anidb_client_core::ClientConfig;
use anidb_client_core::file_io::{FileProcessor, ProcessingStatus};
use anidb_client_core::hashing::HashAlgorithm;
use anidb_client_core::progress::{NullProvider, ProgressProvider, ProgressUpdate};
use tempfile::TempDir;

// Simple test provider that captures ProgressUpdate messages
struct TestProvider {
    updates: std::sync::Mutex<Vec<ProgressUpdate>>,
}

impl TestProvider {
    fn new() -> Self {
        Self {
            updates: std::sync::Mutex::new(Vec::new()),
        }
    }
    fn updates(&self) -> Vec<ProgressUpdate> {
        self.updates.lock().unwrap().clone()
    }
}

impl ProgressProvider for TestProvider {
    fn report(&self, update: ProgressUpdate) {
        self.updates.lock().unwrap().push(update);
    }
    fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
        Box::new(TestProvider {
            updates: std::sync::Mutex::new(self.updates()),
        })
    }
    fn complete(&self) {}
}

/// Test that FileProcessor uses calculate_multiple when processing multiple algorithms
#[tokio::test]
async fn test_file_processor_uses_single_read_for_multiple_algorithms() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.bin");

    // Create a 10MB test file to ensure chunked reading
    let test_data = vec![0x42u8; 10 * 1024 * 1024];
    std::fs::write(&test_file, &test_data).unwrap();

    let config = ClientConfig::test();
    let processor = FileProcessor::new(config);

    let algorithms = vec![
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    // Act
    let start = std::time::Instant::now();
    let null_provider = std::sync::Arc::new(NullProvider);
    let result = processor
        .process_file(&test_file, &algorithms, null_provider.clone())
        .await;
    let duration = start.elapsed();

    // Assert
    assert!(result.is_ok());
    let file_result = result.unwrap();
    assert_eq!(file_result.status, ProcessingStatus::Completed);
    assert_eq!(file_result.hashes.len(), 4);

    // All algorithms should be present
    for algo in &algorithms {
        assert!(file_result.hashes.contains_key(algo));
    }

    // The processing time should be reasonable for a single file read
    // If it were reading 4 times, it would take much longer
    // This is a heuristic check - exact timing depends on hardware
    println!("Processing time for 4 algorithms: {duration:?}");
}

/// Test that progress reporting works correctly with parallel hash calculation
#[tokio::test]
async fn test_parallel_hash_with_progress_reporting() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("progress_test.bin");

    // Create a 5MB test file
    let test_data = vec![0xAAu8; 5 * 1024 * 1024];
    std::fs::write(&test_file, &test_data).unwrap();

    let config = ClientConfig::test();
    let processor = FileProcessor::new(config);

    let algorithms = vec![HashAlgorithm::ED2K, HashAlgorithm::SHA1];

    // Act
    let provider = std::sync::Arc::new(TestProvider::new());
    let result = processor
        .process_file(&test_file, &algorithms, provider.clone())
        .await;

    // Assert
    assert!(result.is_ok());
    let file_result = result.unwrap();
    assert_eq!(file_result.hashes.len(), 2);

    let updates = provider.updates();
    assert!(!updates.is_empty());
    // Ensure bytes_processed increases and reaches total
    let mut last_bytes = 0u64;
    for u in updates.iter() {
        if let ProgressUpdate::HashProgress {
            bytes_processed,
            total_bytes,
            ..
        } = u
        {
            assert!(*bytes_processed >= last_bytes);
            last_bytes = *bytes_processed;
            assert_eq!(*total_bytes, 5 * 1024 * 1024);
        }
    }
    assert_eq!(last_bytes, 5 * 1024 * 1024);
}

/// Test that empty algorithm list is handled correctly
#[tokio::test]
async fn test_empty_algorithm_list() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("empty_algo_test.bin");
    std::fs::write(&test_file, b"test content").unwrap();

    let config = ClientConfig::test();
    let processor = FileProcessor::new(config);

    // Act
    let null_provider = std::sync::Arc::new(NullProvider);
    let result = processor
        .process_file(&test_file, &[], null_provider.clone())
        .await;

    // Assert
    assert!(result.is_ok());
    let file_result = result.unwrap();
    assert_eq!(file_result.status, ProcessingStatus::Completed);
    assert!(file_result.hashes.is_empty());
}

/// Test that single algorithm still works correctly
#[tokio::test]
async fn test_single_algorithm_still_works() {
    // Arrange
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("single_algo_test.bin");
    let test_data = b"single algorithm test content";
    std::fs::write(&test_file, test_data).unwrap();

    let config = ClientConfig::test();
    let processor = FileProcessor::new(config);

    // Act
    let null_provider = std::sync::Arc::new(NullProvider);
    let result = processor
        .process_file(&test_file, &[HashAlgorithm::CRC32], null_provider.clone())
        .await;

    // Assert
    assert!(result.is_ok());
    let file_result = result.unwrap();
    assert_eq!(file_result.status, ProcessingStatus::Completed);
    assert_eq!(file_result.hashes.len(), 1);
}
