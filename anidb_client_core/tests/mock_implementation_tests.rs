//! Tests for mock implementations
//!
//! These tests validate that the mock implementations behave correctly
//! and provide the expected interfaces for dependent teams.

use anidb_client_core::api::{BatchOptions, ProcessOptions};
use anidb_client_core::error::{IoError, ProtocolError};
use anidb_client_core::file_io::ProcessingStatus;
use anidb_client_core::progress::{ChannelAdapter, ProgressProvider};
use anidb_client_core::{Error, HashAlgorithm, Progress};
use anidb_test_utils::builders::test_utils;
use anidb_test_utils::mocks::{MockAniDBClient, MockHashCalculator};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

#[cfg(test)]
mod mock_client_tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_client_creation() {
        let mock_client = MockAniDBClient::new();
        assert!(mock_client.is_ready());
    }

    #[tokio::test]
    async fn test_mock_successful_file_processing() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_file_processing_success();

        let options =
            ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32]);

        let result = mock_client
            .process_file(Path::new("test.mkv"), options)
            .await;

        assert!(result.is_ok());
        let file_result = result.unwrap();
        assert_eq!(file_result.file_path, PathBuf::from("test.mkv"));
        assert_eq!(file_result.status, ProcessingStatus::Completed);
        assert!(file_result.hashes.contains_key(&HashAlgorithm::ED2K));
        assert!(file_result.hashes.contains_key(&HashAlgorithm::CRC32));
    }

    #[tokio::test]
    async fn test_mock_file_processing_error() {
        let mut mock_client = MockAniDBClient::new();
        let expected_error = Error::Io(IoError::file_not_found(&PathBuf::from("missing.mkv")));
        mock_client.expect_file_processing_error(expected_error);

        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let result = mock_client
            .process_file(Path::new("test.mkv"), options)
            .await;

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::Io(io_err) if io_err.kind == anidb_client_core::error::IoErrorKind::FileNotFound)
        );
    }

    #[tokio::test]
    async fn test_mock_progress_reporting() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_file_processing_success();
        mock_client.set_progress_reporting(true);
        mock_client.set_processing_delay(Duration::from_millis(50));

        let (progress_tx, mut progress_rx) = mpsc::channel::<Progress>(100);
        let progress_provider =
            Arc::new(ChannelAdapter::new(progress_tx)) as Arc<dyn ProgressProvider>;
        let options = ProcessOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_progress_reporting(true)
            .with_progress_provider(progress_provider);

        let process_task = tokio::spawn(async move {
            mock_client
                .process_file(Path::new("test.mkv"), options)
                .await
        });

        let mut progress_updates = Vec::new();
        while let Some(progress) = progress_rx.recv().await {
            progress_updates.push(progress.percentage);
            if progress.percentage >= 100.0 {
                break;
            }
        }

        let result = process_task.await.unwrap();
        assert!(result.is_ok());
        assert!(!progress_updates.is_empty());
        assert!(progress_updates.contains(&100.0));
    }

    #[tokio::test]
    async fn test_mock_anime_identification_success() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_anime_identification("One Piece", 1000);

        let result = mock_client.identify_file("test_hash", 1024 * 1024).await;

        assert!(result.is_ok());
        let identification = result.unwrap();
        assert_eq!(identification.title, "One Piece");
        assert_eq!(identification.episode_number, 1000);
    }

    #[tokio::test]
    async fn test_mock_anime_identification_error() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_identification_error(Error::Protocol(ProtocolError::NetworkOffline));

        let result = mock_client.identify_file("test_hash", 1024 * 1024).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Protocol(anidb_client_core::error::ProtocolError::NetworkOffline)
        ));
    }

    #[tokio::test]
    async fn test_mock_batch_all_success() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_batch_all_success();

        let files = vec![
            PathBuf::from("anime1.mkv"),
            PathBuf::from("anime2.mkv"),
            PathBuf::from("anime3.mkv"),
        ];

        let options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(2);

        let result = mock_client.process_batch(&files, options).await;

        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.total_files, 3);
        assert_eq!(batch_result.successful_files, 3);
        assert_eq!(batch_result.failed_files, 0);
    }

    #[tokio::test]
    async fn test_mock_batch_all_failure() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_batch_all_failure();

        let files = vec![PathBuf::from("anime1.mkv"), PathBuf::from("anime2.mkv")];

        let options = BatchOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let result = mock_client.process_batch(&files, options).await;

        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.total_files, 2);
        assert_eq!(batch_result.successful_files, 0);
        assert_eq!(batch_result.failed_files, 2);
    }

    #[tokio::test]
    async fn test_mock_batch_partial_success() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.expect_batch_partial_success(2); // 2 out of 3 succeed

        let files = vec![
            PathBuf::from("anime1.mkv"),
            PathBuf::from("anime2.mkv"),
            PathBuf::from("anime3.mkv"),
        ];

        let options = BatchOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let result = mock_client.process_batch(&files, options).await;

        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.total_files, 3);
        assert_eq!(batch_result.successful_files, 2);
        assert_eq!(batch_result.failed_files, 1);
    }

    #[tokio::test]
    async fn test_mock_batch_custom_results() {
        let mut mock_client = MockAniDBClient::new();
        let custom_results = vec![
            Ok(test_utils::create_mock_file_result(
                PathBuf::from("success1.mkv"),
                &[HashAlgorithm::ED2K],
            )),
            Err(Error::Io(IoError::file_not_found(&PathBuf::from(
                "failure1.mkv",
            )))),
            Ok(test_utils::create_mock_file_result(
                PathBuf::from("success2.mkv"),
                &[HashAlgorithm::CRC32],
            )),
        ];
        mock_client.expect_batch_custom_results(custom_results);

        let files = vec![
            PathBuf::from("anime1.mkv"),
            PathBuf::from("anime2.mkv"),
            PathBuf::from("anime3.mkv"),
        ];

        let options =
            BatchOptions::new().with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32]);

        let result = mock_client.process_batch(&files, options).await;

        assert!(result.is_ok());
        let batch_result = result.unwrap();
        assert_eq!(batch_result.total_files, 3);
        assert_eq!(batch_result.successful_files, 2);
        assert_eq!(batch_result.failed_files, 1);
        assert!(batch_result.results[0].is_ok());
        assert!(batch_result.results[1].is_err());
        assert!(batch_result.results[2].is_ok());
    }
    #[tokio::test]
    async fn test_mock_processing_delay() {
        let mut mock_client = MockAniDBClient::new();
        mock_client.set_processing_delay(Duration::from_millis(100));

        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let start_time = std::time::Instant::now();
        let result = mock_client
            .process_file(Path::new("test.mkv"), options)
            .await;
        let elapsed = start_time.elapsed();

        assert!(result.is_ok());
        assert!(elapsed >= Duration::from_millis(90)); // Allow some variance
    }
}

#[cfg(test)]
mod mock_hash_calculator_tests {
    use super::*;

    #[test]
    fn test_mock_hash_calculator_creation() {
        let calculator = MockHashCalculator::new();

        let hash = calculator.calculate_hash(HashAlgorithm::ED2K, b"test content");
        assert_eq!(hash, "098f6bcd4621d373cade4e832627b4f6");
    }

    #[test]
    fn test_mock_hash_calculator_custom_hash() {
        let mut calculator = MockHashCalculator::new();
        calculator.add_hash(HashAlgorithm::CRC32, b"custom data", "12345678");

        let hash = calculator.calculate_hash(HashAlgorithm::CRC32, b"custom data");
        assert_eq!(hash, "12345678");
    }

    #[test]
    fn test_mock_hash_calculator_fallback() {
        let calculator = MockHashCalculator::new();

        // For unknown data, should return a predictable fallback
        let hash = calculator.calculate_hash(HashAlgorithm::MD5, b"unknown data");
        assert!(hash.starts_with("mock_md5_"));
        assert!(hash.contains(&format!("{:08x}", b"unknown data".len())));
    }

    #[test]
    fn test_mock_hash_calculator_different_algorithms() {
        let calculator = MockHashCalculator::new();

        let ed2k_hash = calculator.calculate_hash(HashAlgorithm::ED2K, b"same data");
        let crc32_hash = calculator.calculate_hash(HashAlgorithm::CRC32, b"same data");
        let md5_hash = calculator.calculate_hash(HashAlgorithm::MD5, b"same data");

        // Different algorithms should produce different results
        assert_ne!(ed2k_hash, crc32_hash);
        assert_ne!(crc32_hash, md5_hash);
        assert_ne!(ed2k_hash, md5_hash);
    }
}

#[cfg(test)]
mod test_utils_tests {
    use super::*;

    #[test]
    fn test_create_mock_file_result() {
        let file_path = PathBuf::from("test_anime.mkv");
        let algorithms = &[HashAlgorithm::ED2K, HashAlgorithm::CRC32];

        let result = test_utils::create_mock_file_result(file_path.clone(), algorithms);

        assert_eq!(result.file_path, file_path);
        assert_eq!(result.file_size, 1024 * 1024);
        assert_eq!(result.status, ProcessingStatus::Completed);
        assert_eq!(result.hashes.len(), 2);
        assert!(result.hashes.contains_key(&HashAlgorithm::ED2K));
        assert!(result.hashes.contains_key(&HashAlgorithm::CRC32));
        assert!(result.anime_info.is_some());
    }

    #[test]
    fn test_create_mock_anime_identification() {
        let identification = test_utils::create_mock_anime_identification("Death Note", 25);

        assert_eq!(identification.title, "Death Note");
        assert_eq!(identification.episode_number, 25);
        assert!(identification.anime_id > 0);
        assert!(identification.episode_id > 0);
    }

    #[test]
    fn test_create_mock_progress() {
        let progress = test_utils::create_mock_progress(75.0, 768 * 1024);

        assert_eq!(progress.percentage, 75.0);
        assert_eq!(progress.bytes_processed, 768 * 1024);
        assert_eq!(progress.total_bytes, 1024 * 1024);
        assert_eq!(progress.throughput_mbps, 100.0);
        assert!(progress.current_operation.contains("75%"));
    }
}

#[cfg(test)]
mod mock_validation_tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_validates_options() {
        let mock_client = MockAniDBClient::new();

        // Empty algorithms should fail validation
        let invalid_options = ProcessOptions::new(); // No algorithms set
        let result = mock_client
            .process_file(Path::new("test.mkv"), invalid_options)
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Validation(
                anidb_client_core::error::ValidationError::InvalidConfiguration { .. }
            )
        ));
    }

    #[tokio::test]
    async fn test_mock_respects_algorithm_filter() {
        let mock_client = MockAniDBClient::new();

        // Request only ED2K
        let options = ProcessOptions::new().with_algorithms(&[HashAlgorithm::ED2K]);

        let result = mock_client
            .process_file(Path::new("test.mkv"), options)
            .await;

        assert!(result.is_ok());
        let file_result = result.unwrap();
        assert_eq!(file_result.hashes.len(), 1);
        assert!(file_result.hashes.contains_key(&HashAlgorithm::ED2K));
        assert!(!file_result.hashes.contains_key(&HashAlgorithm::CRC32));
    }

    #[tokio::test]
    async fn test_mock_batch_validates_options() {
        let mock_client = MockAniDBClient::new();

        let files = vec![PathBuf::from("test.mkv")];

        // Invalid max_concurrent should fail
        let invalid_options = BatchOptions::new()
            .with_algorithms(&[HashAlgorithm::ED2K])
            .with_max_concurrent(0); // Invalid

        let result = mock_client.process_batch(&files, invalid_options).await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Error::Validation(
                anidb_client_core::error::ValidationError::InvalidConfiguration { .. }
            )
        ));
    }
}
