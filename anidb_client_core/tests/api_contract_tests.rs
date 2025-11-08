//! API Contract Tests for Core API Design (CORE-003)
//!
//! These tests define the expected behavior of the public API interface.
//! Following TDD principles, these tests are written first (RED phase) to define
//! the contracts that the implementation must satisfy.

use anidb_client_core::error::IoError;
use anidb_client_core::progress::{ProgressProvider, ProgressUpdate};
use anidb_client_core::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test the main AniDBClient struct and its core operations
#[cfg(test)]
mod anidb_client_tests {
    use super::*;

    /// Test that AniDBClient can be created with configuration
    #[tokio::test]
    async fn test_anidb_client_creation_with_config() {
        // Arrange
        let config = ClientConfig::test();

        // Act
        let result = AniDBClient::new(config).await;

        // Assert
        assert!(result.is_ok());
        let client = result.unwrap();
        assert!(client.is_ready());
    }

    /// Test processing a single file with specified options
    #[tokio::test]
    async fn test_process_file_with_options() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mkv");
        std::fs::write(&test_file, b"test video content").unwrap();

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        let options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32])
            .with_progress_reporting(true);

        // Act
        let result = client.process_file(&test_file, options).await;

        // Assert
        assert!(result.is_ok());
        let file_result = result.unwrap();
        assert_eq!(file_result.file_path, test_file);
        assert!(file_result.hashes.contains_key(&HashAlgorithm::ED2K));
        assert!(file_result.hashes.contains_key(&HashAlgorithm::CRC32));
        assert_eq!(file_result.status, ProcessingStatus::Completed);
    }

    /// Test batch processing multiple files
    #[tokio::test]
    async fn test_process_batch_files() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_files = vec![
            temp_dir.path().join("anime1.mkv"),
            temp_dir.path().join("anime2.mkv"),
            temp_dir.path().join("anime3.mkv"),
        ];

        for file in &test_files {
            std::fs::write(file, b"test video content").unwrap();
        }

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(2)
            .with_continue_on_error(true);

        // Act
        let result = client.process_batch(&test_files, options).await;

        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.total_files, 3);
        assert_eq!(batch_result.successful_files, 3);
        assert_eq!(batch_result.failed_files, 0);
        assert_eq!(batch_result.results.len(), 3);
    }

    /// Test file identification using hash and size
    #[tokio::test]
    async fn test_identify_file_by_hash() {
        // Arrange
        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        let test_hash = "098f6bcd4621d373cade4e832627b4f6";
        let file_size = 1024u64;

        // Act
        let result = client.identify_file(test_hash, file_size).await;

        // Assert
        // For now, we expect this to work without actual network calls in test mode
        assert!(
            result.is_ok()
                || matches!(
                    result,
                    Err(Error::Protocol(
                        anidb_client_core::error::ProtocolError::NetworkOffline
                    ))
                )
        );
    }

    /// Test progress reporting during file processing
    #[tokio::test]
    async fn test_progress_reporting() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("large_test.mkv");
        let large_content = vec![0u8; 10 * 1024 * 1024]; // 10MB
        std::fs::write(&test_file, &large_content).unwrap();

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        // Simple capturing provider
        struct TestProvider {
            updates: std::sync::Mutex<Vec<ProgressUpdate>>,
        }
        impl TestProvider {
            fn new() -> Self {
                Self {
                    updates: std::sync::Mutex::new(Vec::new()),
                }
            }
        }
        impl ProgressProvider for TestProvider {
            fn report(&self, update: ProgressUpdate) {
                self.updates.lock().unwrap().push(update);
            }
            fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
                Box::new(TestProvider::new())
            }
            fn complete(&self) {}
        }

        let provider = std::sync::Arc::new(TestProvider::new());
        let options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_progress_reporting(true);

        let result = client
            .process_file_with_progress(&test_file, options, provider.clone())
            .await;

        // Assert
        assert!(result.is_ok());
        let ups = provider.updates.lock().unwrap();
        assert!(!ups.is_empty());
        // Ensure we reached total bytes
        let mut reached_end = false;
        for u in ups.iter() {
            if let ProgressUpdate::HashProgress {
                bytes_processed,
                total_bytes,
                ..
            } = u
                && *bytes_processed == *total_bytes
            {
                reached_end = true;
            }
        }
        assert!(reached_end);
    }

    /// Test error handling for invalid files
    #[tokio::test]
    async fn test_error_handling_invalid_file() {
        // Arrange
        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        let non_existent_file = PathBuf::from("/path/to/non/existent/file.mkv");
        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        // Act
        let result = client.process_file(&non_existent_file, options).await;

        // Assert
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::Io(io_err) if io_err.kind == anidb_client_core::error::IoErrorKind::FileNotFound)
        );
    }
}

/// Test ProcessOptions configuration struct
#[cfg(test)]
mod process_options_tests {
    use super::*;

    #[test]
    fn test_process_options_creation_and_configuration() {
        // Act
        let options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32])
            .with_progress_reporting(true);

        // Assert
        assert_eq!(options.algorithms().len(), 2);
        assert!(options.algorithms().contains(&HashAlgorithm::ED2K));
        assert!(options.algorithms().contains(&HashAlgorithm::CRC32));
        assert!(options.progress_reporting());
    }

    #[test]
    fn test_process_options_validation() {
        // Test that empty algorithms list is invalid
        let result = ProcessOptions::new().validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Validation(
                anidb_client_core::error::ValidationError::InvalidConfiguration { .. }
            )
        ));

        // Test that valid configuration passes validation
        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);
        assert!(options.validate().is_ok());
    }
}

/// Test BatchOptions configuration struct
#[cfg(test)]
mod batch_options_tests {
    use super::*;

    #[test]
    fn test_batch_options_creation_and_configuration() {
        // Act
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(4)
            .with_continue_on_error(true)
            .with_skip_existing(false);

        // Assert
        assert_eq!(options.algorithms().len(), 1);
        assert_eq!(options.max_concurrent(), 4);
        assert!(options.continue_on_error());
        assert!(!options.skip_existing());
    }

    #[test]
    fn test_batch_options_validation() {
        // Test invalid max_concurrent
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(0);
        let result = options.validate();
        assert!(result.is_err());

        // Test valid configuration
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(2);
        assert!(options.validate().is_ok());
    }
}

/// Test FileResult and BatchResult structures
#[cfg(test)]
mod result_types_tests {
    use super::*;

    #[test]
    fn test_file_result_creation() {
        // Arrange
        let mut hashes = HashMap::new();
        hashes.insert(HashAlgorithm::ED2K, "test_hash".to_string());

        // Act
        let result = FileResult {
            file_path: PathBuf::from("test.mkv"),
            file_size: 1024,
            hashes,
            status: ProcessingStatus::Completed,
            processing_time: std::time::Duration::from_secs(5),
            anime_info: None,
        };

        // Assert
        assert_eq!(result.file_path, PathBuf::from("test.mkv"));
        assert_eq!(result.file_size, 1024);
        assert_eq!(result.status, ProcessingStatus::Completed);
        assert!(result.hashes.contains_key(&HashAlgorithm::ED2K));
    }

    #[test]
    fn test_batch_result_statistics() {
        // Arrange
        let results = vec![
            Ok(create_test_file_result(ProcessingStatus::Completed)),
            Ok(create_test_file_result(ProcessingStatus::Completed)),
            Err(Error::Io(IoError::file_not_found(&PathBuf::from(
                "missing.mkv",
            )))),
        ];

        // Act
        let batch_result = BatchResult::from_results(results);

        // Assert
        assert_eq!(batch_result.total_files, 3);
        assert_eq!(batch_result.successful_files, 2);
        assert_eq!(batch_result.failed_files, 1);
    }

    fn create_test_file_result(status: ProcessingStatus) -> FileResult {
        FileResult {
            file_path: PathBuf::from("test.mkv"),
            file_size: 1024,
            hashes: HashMap::new(),
            status,
            processing_time: std::time::Duration::from_secs(1),
            anime_info: None,
        }
    }
}

/// Test anime identification structures
#[cfg(test)]
mod identification_tests {
    use super::*;

    #[test]
    fn test_anime_identification_creation() {
        // Act
        let identification = AnimeIdentification {
            anime_id: 123,
            episode_id: 456,
            title: "One Piece".to_string(),
            episode_number: 1000,
            source: IdentificationSource::AniDB,
        };

        // Assert
        assert_eq!(identification.anime_id, 123);
        assert_eq!(identification.episode_id, 456);
        assert_eq!(identification.title, "One Piece");
        assert_eq!(identification.episode_number, 1000);
        assert_eq!(identification.source, IdentificationSource::AniDB);
    }
}
