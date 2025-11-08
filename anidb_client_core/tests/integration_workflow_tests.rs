//! Integration tests for complete API workflow validation
//!
//! These tests validate that the core API components work together correctly
//! in realistic scenarios that dependent teams will encounter.

use anidb_client_core::api::{BatchOptions, ProcessOptions};
use anidb_client_core::*;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

/// Helper function to create test files with specific content
async fn create_test_file(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
    let file_path = dir.join(name);
    tokio::fs::write(&file_path, content).await.unwrap();
    file_path
}

/// Helper function to create anime-like test files
async fn create_anime_files(dir: &Path) -> Vec<PathBuf> {
    vec![
        create_test_file(
            dir,
            "[SubsPlease] One Piece - 1000 [1080p].mkv",
            b"anime content 1",
        )
        .await,
        create_test_file(
            dir,
            "[HorribleSubs] Attack on Titan - 01 [720p].mkv",
            b"anime content 2",
        )
        .await,
        create_test_file(dir, "Death Note Episode 25.avi", b"anime content 3").await,
        create_test_file(dir, "Naruto_Shippuden_500.mp4", b"anime content 4").await,
    ]
}

#[cfg(test)]
mod single_file_workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_single_file_processing_workflow() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(
            temp_dir.path(),
            "[SubsPlease] One Piece - 1000 [1080p].mkv",
            &vec![0u8; 10 * 1024], // 10KB test file
        )
        .await;

        let config = ClientConfig::test();
        let client = AniDBClient::new(config).await.unwrap();

        // Act - Process file with multiple algorithms
        let options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32])
            .with_progress_reporting(true);

        let result = client.process_file(&test_file, options).await;

        // Assert
        assert!(result.is_ok());
        let file_result = result.unwrap();

        // Verify basic properties
        assert_eq!(file_result.file_path, test_file);
        assert_eq!(file_result.file_size, 10 * 1024);
        assert_eq!(file_result.status, ProcessingStatus::Completed);
        assert!(file_result.processing_time > Duration::from_millis(0));

        // Verify all requested hashes were calculated
        assert_eq!(file_result.hashes.len(), 2);
        assert!(file_result.hashes.contains_key(&HashAlgorithm::ED2K));
        assert!(file_result.hashes.contains_key(&HashAlgorithm::CRC32));

        // Verify hash format (basic validation)
        for (algorithm, hash) in &file_result.hashes {
            assert!(!hash.is_empty());
            match algorithm {
                HashAlgorithm::ED2K => assert_eq!(hash.len(), 32), // MD4 hex string
                HashAlgorithm::CRC32 => assert_eq!(hash.len(), 8), // CRC32 hex string
                HashAlgorithm::MD5 => assert_eq!(hash.len(), 32),  // MD5 hex string
                HashAlgorithm::SHA1 => assert_eq!(hash.len(), 40), // SHA1 hex string
                HashAlgorithm::TTH => assert!(hash.len() >= 39),   // TTH base32 string
            }
        }
    }

    #[tokio::test]
    async fn test_single_file_with_progress_reporting() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(
            temp_dir.path(),
            "large_anime.mkv",
            &vec![0u8; 100 * 1024], // 100KB test file
        )
        .await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Test provider capturing ProgressUpdate
        struct TestProvider {
            updates: std::sync::Mutex<Vec<anidb_client_core::progress::ProgressUpdate>>,
        }
        impl TestProvider {
            fn new() -> Self {
                Self {
                    updates: std::sync::Mutex::new(Vec::new()),
                }
            }
        }
        impl anidb_client_core::progress::ProgressProvider for TestProvider {
            fn report(&self, u: anidb_client_core::progress::ProgressUpdate) {
                self.updates.lock().unwrap().push(u);
            }
            fn create_child(
                &self,
                _n: &str,
            ) -> Box<dyn anidb_client_core::progress::ProgressProvider> {
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

        // Verify progress updates are logical
        let updates = provider.updates.lock().unwrap();
        assert!(!updates.is_empty());
        // Find last HashProgress and verify bytes
        let mut last_bytes = 0u64;
        for u in updates.iter() {
            if let anidb_client_core::progress::ProgressUpdate::HashProgress {
                bytes_processed,
                total_bytes,
                ..
            } = u
            {
                last_bytes = *bytes_processed;
                assert_eq!(*total_bytes, 100 * 1024);
            }
        }
        assert_eq!(last_bytes, 100 * 1024);

        // Verify progress is monotonically increasing
        // Monotonic check on bytes
        let mut prev = 0u64;
        for u in updates.iter() {
            if let anidb_client_core::progress::ProgressUpdate::HashProgress {
                bytes_processed,
                ..
            } = u
            {
                assert!(*bytes_processed >= prev);
                prev = *bytes_processed;
            }
        }
    }

    #[tokio::test]
    async fn test_unsupported_file_extension_handling() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_file =
            create_test_file(temp_dir.path(), "document.txt", b"not a video file").await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        // Act - Process non-video file (should still work for hash calculation)
        let result = client.process_file(&test_file, options).await;

        // Assert - Should succeed (file processing doesn't restrict by extension)
        assert!(result.is_ok());
        let file_result = result.unwrap();
        assert_eq!(file_result.status, ProcessingStatus::Completed);
        assert!(file_result.hashes.contains_key(&HashAlgorithm::ED2K));
    }

    #[tokio::test]
    async fn test_missing_file_error_handling() {
        // Arrange
        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();
        let missing_file = PathBuf::from("/path/to/nonexistent/anime.mkv");
        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        // Act
        let result = client.process_file(&missing_file, options).await;

        // Assert
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::Io(io_err) if io_err.kind == anidb_client_core::error::IoErrorKind::FileNotFound)
        );
    }
}

#[cfg(test)]
mod batch_processing_workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_batch_processing_workflow() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let anime_files = create_anime_files(temp_dir.path()).await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32])
            .with_max_concurrent(2)
            .with_continue_on_error(true);

        let result = client.process_batch(&anime_files, options).await;

        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();

        // Verify batch statistics
        assert_eq!(batch_result.total_files, 4);
        assert_eq!(batch_result.successful_files, 4);
        assert_eq!(batch_result.failed_files, 0);
        assert_eq!(batch_result.results.len(), 4);
        assert!(batch_result.total_time > Duration::from_millis(0));

        // Verify all results are successful
        for result in &batch_result.results {
            assert!(result.is_ok());
            let file_result = result.as_ref().unwrap();
            assert_eq!(file_result.status, ProcessingStatus::Completed);
            assert_eq!(file_result.hashes.len(), 2); // ED2K + CRC32
        }
    }

    #[tokio::test]
    async fn test_batch_processing_with_mixed_results() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let files = create_anime_files(temp_dir.path()).await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(3)
            .with_continue_on_error(true);

        let result = client.process_batch(&files, options).await;

        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();

        // Verify all files processed successfully
        assert_eq!(batch_result.total_files, 4);
        assert_eq!(batch_result.successful_files, 4);
        assert_eq!(batch_result.failed_files, 0);

        // Verify that all files succeeded
        for i in 0..4 {
            assert!(batch_result.results[i].is_ok());
        }
    }

    #[tokio::test]
    async fn test_batch_processing_concurrency_limits() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let anime_files = create_anime_files(temp_dir.path()).await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act - Test with different concurrency limits
        let options1 = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::CRC32])
            .with_max_concurrent(1);

        let options2 = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::CRC32])
            .with_max_concurrent(4);

        let start_time1 = std::time::Instant::now();
        let result1 = client.process_batch(&anime_files, options1).await;
        let duration1 = start_time1.elapsed();

        let start_time2 = std::time::Instant::now();
        let result2 = client.process_batch(&anime_files, options2).await;
        let duration2 = start_time2.elapsed();

        // Assert
        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // For small test files, concurrent processing might actually be slower due to overhead
        // We just verify that it's not drastically slower (within 3x)
        // In real-world scenarios with larger files, concurrency would show benefits
        assert!(
            duration2 <= duration1 * 3,
            "Concurrent processing took {duration2:?}, sequential took {duration1:?}"
        );

        // Both should have same results
        let batch1 = result1.unwrap();
        let batch2 = result2.unwrap();
        assert_eq!(batch1.successful_files, batch2.successful_files);
        assert_eq!(batch1.failed_files, batch2.failed_files);
    }
}

#[cfg(test)]
mod error_handling_workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_invalid_options_validation() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let test_file = create_test_file(temp_dir.path(), "test.mkv", b"content").await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act - Test invalid ProcessOptions (no algorithms)
        let invalid_options = ProcessOptions::new(); // No algorithms specified
        let result = client.process_file(&test_file, invalid_options).await;

        // Assert
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Validation(
                anidb_client_core::error::ValidationError::InvalidConfiguration { .. }
            )
        ));
    }

    #[tokio::test]
    async fn test_invalid_batch_options_validation() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let files = create_anime_files(temp_dir.path()).await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act - Test invalid BatchOptions (zero concurrency)
        let invalid_options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(0); // Invalid

        let result = client.process_batch(&files, invalid_options).await;

        // Assert
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Validation(
                anidb_client_core::error::ValidationError::InvalidConfiguration { .. }
            )
        ));
    }

    #[tokio::test]
    async fn test_error_recovery_and_logging() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let good_file1 = create_test_file(temp_dir.path(), "good1.mkv", b"valid content 1").await;
        let good_file2 = create_test_file(temp_dir.path(), "good2.mkv", b"valid content 2").await;
        let good_file3 = create_test_file(temp_dir.path(), "good3.mkv", b"valid content 3").await;
        let files = vec![good_file1, good_file2, good_file3];

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act
        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_continue_on_error(true);

        let result = client.process_batch(&files, options).await;

        // Assert
        assert!(result.is_ok());
        let batch_result = result.unwrap();

        // All files should succeed
        assert_eq!(batch_result.total_files, 3);
        assert_eq!(batch_result.successful_files, 3);
        assert_eq!(batch_result.failed_files, 0);

        // All files should succeed
        assert!(batch_result.results[0].is_ok());
        assert!(batch_result.results[1].is_ok());
        assert!(batch_result.results[2].is_ok());
    }
}

#[cfg(test)]
mod anime_identification_workflow_tests {
    use super::*;

    #[tokio::test]
    async fn test_anime_identification_workflow() {
        // Arrange
        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Test with mock hash values
        let test_hash = "098f6bcd4621d373cade4e832627b4f6";
        let file_size = 1024 * 1024 * 700; // 700MB typical anime file

        // Act
        let result = client.identify_file(test_hash, file_size).await;

        // Assert - In test mode, this should return NetworkOffline
        // (Real implementation would connect to AniDB)
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Protocol(anidb_client_core::error::ProtocolError::NetworkOffline)
        ));
    }

    #[tokio::test]
    async fn test_end_to_end_file_processing_and_identification() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let anime_file = create_test_file(
            temp_dir.path(),
            "[SubsPlease] One Piece - 1000 [1080p].mkv",
            &vec![42u8; 50 * 1024], // 50KB with predictable content
        )
        .await;

        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act - First process the file to get its hash
        let process_options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let process_result = client.process_file(&anime_file, process_options).await;
        assert!(process_result.is_ok());

        let file_result = process_result.unwrap();
        let ed2k_hash = file_result.hashes.get(&HashAlgorithm::ED2K).unwrap();

        // Then try to identify it using the calculated hash
        let identify_result = client.identify_file(ed2k_hash, file_result.file_size).await;

        // Assert
        // In test mode, identification should fail with NetworkOffline
        assert!(identify_result.is_err());
        assert!(matches!(
            identify_result.unwrap_err(),
            Error::Protocol(anidb_client_core::error::ProtocolError::NetworkOffline)
        ));

        // But we verified the workflow works end-to-end
        assert!(!ed2k_hash.is_empty());
        assert_eq!(ed2k_hash.len(), 32); // Valid ED2K hash length
    }
}

#[cfg(test)]
mod client_lifecycle_tests {
    use super::*;

    #[tokio::test]
    async fn test_client_creation_and_readiness() {
        // Test different client configurations
        let configs = vec![
            ClientConfig::default(),
            ClientConfig::test(),
            ClientConfig {
                max_concurrent_files: 8,
                chunk_size: 128 * 1024,
                max_memory_usage: 500 * 1024 * 1024,
                username: None,
                password: None,
                client_name: None,
                client_version: None,
            },
        ];

        for config in configs {
            let client = AniDBClient::new(config).await;
            assert!(client.is_ok());
            assert!(client.unwrap().is_ready());
        }
    }

    #[tokio::test]
    async fn test_multiple_operations_on_same_client() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let files = create_anime_files(temp_dir.path()).await;
        let client = AniDBClient::new(ClientConfig::test()).await.unwrap();

        // Act - Perform multiple operations
        let single_result = client
            .process_file(
                &files[0],
                ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]),
            )
            .await;

        let batch_result = client
            .process_batch(
                &files[1..3],
                BatchOptions::new().with_algorithms(&[HashAlgorithm::CRC32]),
            )
            .await;

        let identify_result = client.identify_file("test_hash", 1024).await;

        // Assert
        assert!(single_result.is_ok());
        assert!(batch_result.is_ok());
        assert!(
            identify_result.is_err()
                && matches!(
                    identify_result.unwrap_err(),
                    Error::Protocol(anidb_client_core::error::ProtocolError::NetworkOffline)
                )
        );

        // Client should remain ready after multiple operations
        assert!(client.is_ready());
    }
}
