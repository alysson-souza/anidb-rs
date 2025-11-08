//! Tests for the testing infrastructure itself (TDD)
//!
//! These tests validate that our testing infrastructure components work correctly
//! and provide reliable tools for testing the core library.

use anidb_client_core::error::ValidationError;
use anidb_client_core::{Error, HashAlgorithm};
use anidb_test_utils::builders::{TestDataBuilder, TestFileBuilder as TestFileGenerator};
use anidb_test_utils::mocks::MockFileSystem;
use anidb_test_utils::performance::{CoverageReporter, PerformanceTracker, TestHarness};
use std::path::PathBuf;
use tempfile::TempDir;

/// Test infrastructure module tests
#[cfg(test)]
mod test_file_generator_tests {
    use super::*;

    #[test]
    fn test_create_test_file_generator() {
        // This test will fail initially - we need to implement TestFileGenerator
        let temp_dir = TempDir::new().unwrap();
        let generator = TestFileGenerator::new(temp_dir.path());

        assert!(generator.is_ready());
    }

    #[test]
    fn test_generate_deterministic_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut generator = TestFileGenerator::new(temp_dir.path());

        // Generate same file twice - should be identical
        let file1 = generator
            .generate_deterministic_file("test.mkv", 1024, 12345)
            .unwrap();
        let file2 = generator
            .generate_deterministic_file("test2.mkv", 1024, 12345)
            .unwrap();

        // Files should have same content (different names)
        let content1 = std::fs::read(&file1).unwrap();
        let content2 = std::fs::read(&file2).unwrap();
        assert_eq!(content1, content2);
        assert_eq!(content1.len(), 1024);
    }

    #[test]
    fn test_generate_file_with_known_hash() {
        let temp_dir = TempDir::new().unwrap();
        let mut generator = TestFileGenerator::new(temp_dir.path());

        // Generate file that should produce specific ED2K hash
        let file = generator
            .generate_file_with_hash(
                "ed2k_test.mkv",
                HashAlgorithm::ED2K,
                "d41d8cd98f00b204e9800998ecf8427e",
            )
            .unwrap();

        assert!(file.exists());
        assert_eq!(std::fs::read(&file).unwrap(), b""); // Empty file for this hash
    }

    #[test]
    fn test_generate_large_test_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut generator = TestFileGenerator::new(temp_dir.path());

        // Generate 100MB file
        let size = 100 * 1024 * 1024;
        let file = generator.generate_test_file("large.mkv", size).unwrap();

        assert!(file.exists());
        assert_eq!(std::fs::metadata(&file).unwrap().len(), size as u64);
    }

    #[test]
    fn test_generate_corrupted_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut generator = TestFileGenerator::new(temp_dir.path());

        let file = generator
            .generate_corrupted_file("corrupted.mkv", 1024)
            .unwrap();

        assert!(file.exists());
        // Should have invalid structure but be readable
        assert!(std::fs::read(&file).is_ok());
    }

    #[test]
    fn test_cleanup_test_files() {
        let temp_dir = TempDir::new().unwrap();
        let mut generator = TestFileGenerator::new(temp_dir.path());

        let _file1 = generator.generate_test_file("test1.mkv", 1024).unwrap();
        let _file2 = generator.generate_test_file("test2.mkv", 2048).unwrap();

        // Cleanup should remove all generated files
        generator.cleanup();

        assert_eq!(std::fs::read_dir(temp_dir.path()).unwrap().count(), 0);
    }
}

#[cfg(test)]
mod test_data_builder_tests {
    use super::*;

    #[test]
    fn test_create_test_data_builder() {
        let builder = TestDataBuilder::new();
        let builder_ptr: *const TestDataBuilder = &builder;
        assert!(!builder_ptr.is_null());
    }

    #[test]
    fn test_build_anime_file_data() {
        let data = TestDataBuilder::new()
            .with_anime_title("One Piece")
            .with_episode_number(1000)
            .with_file_size(1024 * 1024 * 1024) // 1GB
            .with_hash(HashAlgorithm::ED2K, "expected_hash")
            .build();

        assert_eq!(data.anime_title, "One Piece");
        assert_eq!(data.episode_number, 1000);
        assert_eq!(data.file_size, 1024 * 1024 * 1024);
        assert_eq!(
            data.expected_hashes.get(&HashAlgorithm::ED2K).unwrap(),
            "expected_hash"
        );
    }

    #[test]
    fn test_build_batch_test_data() {
        let batch = TestDataBuilder::new()
            .create_batch()
            .add_anime_file("Episode 1", 1, 500_000_000)
            .add_anime_file("Episode 2", 2, 600_000_000)
            .add_anime_file("Episode 3", 3, 550_000_000)
            .build_batch();

        assert_eq!(batch.files.len(), 3);
        assert_eq!(batch.total_size, 1_650_000_000);
    }

    #[test]
    fn test_build_error_scenarios() {
        let scenarios = TestDataBuilder::new()
            .create_error_scenarios()
            .add_file_not_found_scenario("/missing/file.mkv")
            .add_permission_denied_scenario("/root/protected.mkv")
            .add_network_error_scenario()
            .build_scenarios();

        assert_eq!(scenarios.len(), 3);
        assert!(
            scenarios
                .iter()
                .any(|s| matches!(s, Error::Io(io_err) if io_err.kind == anidb_client_core::error::IoErrorKind::FileNotFound))
        );
        assert!(
            scenarios
                .iter()
                .any(|s| matches!(s, Error::Io(io_err) if io_err.kind == anidb_client_core::error::IoErrorKind::PermissionDenied))
        );
        assert!(scenarios.iter().any(|s| matches!(
            s,
            Error::Protocol(anidb_client_core::error::ProtocolError::NetworkOffline)
        )));
    }
}

#[cfg(test)]
mod mock_file_system_tests {
    use super::*;

    #[test]
    fn test_create_mock_file_system() {
        let mock_fs = MockFileSystem::new();
        assert!(mock_fs.is_empty());
    }

    #[test]
    fn test_add_mock_file() {
        let mut mock_fs = MockFileSystem::new();

        mock_fs.add_file("/test/anime.mkv", b"test content", None);

        assert!(!mock_fs.is_empty());
        assert!(mock_fs.file_exists("/test/anime.mkv"));
    }

    #[test]
    fn test_read_mock_file() {
        let mut mock_fs = MockFileSystem::new();
        let content = b"test anime file content";

        mock_fs.add_file("/anime/episode1.mkv", content, None);

        let read_content = mock_fs.read_file("/anime/episode1.mkv").unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_mock_file_metadata() {
        let mut mock_fs = MockFileSystem::new();
        let content = b"content";

        mock_fs.add_file_with_metadata("/test.mkv", content, 12345, 1024 * 1024);

        let metadata = mock_fs.get_metadata("/test.mkv").unwrap();
        assert_eq!(metadata.size, content.len() as u64);
        assert_eq!(metadata.created_timestamp, 12345);
        assert_eq!(metadata.modified_timestamp, 1024 * 1024);
    }

    #[test]
    fn test_mock_directory_operations() {
        let mut mock_fs = MockFileSystem::new();

        mock_fs.create_directory("/anime");
        mock_fs.add_file("/anime/ep1.mkv", b"episode 1", None);
        mock_fs.add_file("/anime/ep2.mkv", b"episode 2", None);

        let entries = mock_fs.list_directory("/anime").unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&PathBuf::from("/anime/ep1.mkv")));
        assert!(entries.contains(&PathBuf::from("/anime/ep2.mkv")));
    }

    #[test]
    fn test_mock_file_errors() {
        let mock_fs = MockFileSystem::new();

        // File not found
        let result = mock_fs.read_file("/nonexistent.mkv");
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), Error::Io(io_err) if io_err.kind == anidb_client_core::error::IoErrorKind::FileNotFound)
        );

        // Directory not found
        let result = mock_fs.list_directory("/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_file_system_reset() {
        let mut mock_fs = MockFileSystem::new();

        mock_fs.add_file("/test1.mkv", b"content1", None);
        mock_fs.add_file("/test2.mkv", b"content2", None);
        assert!(!mock_fs.is_empty());

        mock_fs.reset();
        assert!(mock_fs.is_empty());
        assert!(!mock_fs.file_exists("/test1.mkv"));
    }
}

#[cfg(test)]
mod performance_tracker_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_create_performance_tracker() {
        let tracker = PerformanceTracker::new();
        assert_eq!(tracker.get_baseline_count(), 0);
    }

    #[test]
    fn test_track_operation_performance() {
        let mut tracker = PerformanceTracker::new();

        let operation_id = tracker.start_tracking("ed2k_hash_1mb");
        std::thread::sleep(Duration::from_millis(10));
        tracker.finish_tracking(operation_id);

        let metrics = tracker.get_metrics("ed2k_hash_1mb").unwrap();
        assert!(metrics.duration >= Duration::from_millis(10));
        assert!(metrics.memory_usage.is_some());
    }

    #[test]
    fn test_establish_baseline() {
        let mut tracker = PerformanceTracker::new();

        // Run operation multiple times to establish baseline
        for _ in 0..5 {
            let op_id = tracker.start_tracking("test_operation");
            std::thread::sleep(Duration::from_millis(5));
            tracker.finish_tracking(op_id);
        }

        tracker.establish_baseline("test_operation");

        let baseline = tracker.get_baseline("test_operation").unwrap();
        assert!(baseline.average_duration >= Duration::from_millis(5));
        assert!(baseline.min_duration <= baseline.max_duration);
    }

    #[test]
    fn test_performance_regression_detection() {
        let mut tracker = PerformanceTracker::new();

        // Establish baseline with fast operations
        for _ in 0..3 {
            let op_id = tracker.start_tracking("fast_op");
            std::thread::sleep(Duration::from_millis(1));
            tracker.finish_tracking(op_id);
        }
        tracker.establish_baseline("fast_op");

        // Run slower operation
        let op_id = tracker.start_tracking("fast_op");
        std::thread::sleep(Duration::from_millis(50)); // Much slower
        tracker.finish_tracking(op_id);

        let regression = tracker.check_regression("fast_op", 2.0); // 100% regression threshold
        assert!(regression.is_some());
        assert!(regression.unwrap().regression_factor > 2.0);
    }

    #[test]
    fn test_memory_usage_tracking() {
        let mut tracker = PerformanceTracker::new();

        let op_id = tracker.start_tracking("memory_test");

        // Simulate memory allocation
        let _large_vec: Vec<u8> = vec![0; 1024 * 1024]; // 1MB

        tracker.finish_tracking(op_id);

        let metrics = tracker.get_metrics("memory_test").unwrap();
        assert!(metrics.memory_usage.is_some());
        assert!(metrics.peak_memory.is_some());
    }
}

#[cfg(test)]
mod coverage_reporter_tests {
    use super::*;

    #[test]
    fn test_create_coverage_reporter() {
        let reporter = CoverageReporter::new();
        assert_eq!(reporter.get_overall_coverage(), 0.0);
    }

    #[test]
    fn test_track_module_coverage() {
        let mut reporter = CoverageReporter::new();

        reporter.add_module_coverage("file_io", 95.5);
        reporter.add_module_coverage("hashing", 98.2);
        reporter.add_module_coverage("api", 87.1);

        assert_eq!(reporter.get_module_coverage("file_io").unwrap(), 95.5);
        assert_eq!(reporter.get_module_coverage("hashing").unwrap(), 98.2);

        let overall = reporter.get_overall_coverage();
        assert!(overall > 90.0 && overall < 95.0);
    }

    #[test]
    fn test_coverage_thresholds() {
        let mut reporter = CoverageReporter::new();

        reporter.set_threshold("unit_tests", 95.0);
        reporter.set_threshold("integration_tests", 85.0);

        reporter.add_coverage("unit_tests", 96.5);
        reporter.add_coverage("integration_tests", 82.3);

        assert!(reporter.meets_threshold("unit_tests"));
        assert!(!reporter.meets_threshold("integration_tests"));
    }

    #[test]
    fn test_generate_coverage_report() {
        let mut reporter = CoverageReporter::new();

        reporter.add_module_coverage("core", 94.2);
        reporter.add_module_coverage("hash", 99.1);
        reporter.add_module_coverage("network", 88.5);

        let report = reporter.generate_report();

        assert!(report.contains("core: 94.2%"));
        assert!(report.contains("hash: 99.1%"));
        assert!(report.contains("network: 88.5%"));
        assert!(report.contains("Overall:"));
    }
}

#[cfg(test)]
mod test_harness_tests {
    use super::*;

    #[test]
    fn test_create_test_harness() {
        let harness = TestHarness::new();
        assert!(harness.is_ready());
    }

    #[test]
    fn test_run_test_suite() {
        let mut harness = TestHarness::new();

        // Add test cases
        harness.add_test_case(
            "file_processing",
            Box::new(|| {
                // Mock test case
                Ok(())
            }),
        );

        harness.add_test_case(
            "hash_calculation",
            Box::new(|| {
                // Mock test case
                Ok(())
            }),
        );

        let results = harness.run_all_tests();

        assert_eq!(results.total_tests, 2);
        assert_eq!(results.passed_tests, 2);
        assert_eq!(results.failed_tests, 0);
    }

    #[test]
    fn test_test_harness_with_failures() {
        let mut harness = TestHarness::new();

        harness.add_test_case("passing_test", Box::new(|| Ok(())));
        harness.add_test_case(
            "failing_test",
            Box::new(|| {
                Err(Error::Validation(ValidationError::invalid_configuration(
                    "Test failure",
                )))
            }),
        );

        let results = harness.run_all_tests();

        assert_eq!(results.total_tests, 2);
        assert_eq!(results.passed_tests, 1);
        assert_eq!(results.failed_tests, 1);
    }

    #[test]
    fn test_integration_test_setup() {
        let mut harness = TestHarness::new();

        // Setup integration test environment
        harness.setup_integration_environment();

        assert!(harness.has_mock_file_system());
        assert!(harness.has_test_data_generator());
        assert!(harness.has_performance_tracker());
    }

    #[test]
    fn test_benchmark_integration() {
        let mut harness = TestHarness::new();

        harness.add_benchmark(
            "ed2k_1mb",
            Box::new(|| {
                // Mock benchmark
                std::thread::sleep(std::time::Duration::from_millis(10));
                Ok(())
            }),
        );

        let benchmark_results = harness.run_benchmarks();

        assert_eq!(benchmark_results.len(), 1);
        assert!(benchmark_results.contains_key("ed2k_1mb"));
    }
}
