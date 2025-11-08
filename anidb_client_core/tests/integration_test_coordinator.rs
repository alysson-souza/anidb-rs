//! Integration tests using the FileOperationsTestHarness
//!
//! This module uses the test harness from tests/common to run
//! scenario-based integration tests.

mod common;

use common::FileOperationsTestHarness;
use common::test_harness::{ContentType, ExpectedOutcome, TestData, TestScenario, TestType};

#[tokio::test]
async fn test_file_operations_standard_scenarios() {
    let mut harness = FileOperationsTestHarness::new().expect("Failed to create test harness");

    // Setup all standard test scenarios
    harness.setup_standard_scenarios();

    // Run all scenarios and collect results
    let results = harness.run_all_scenarios().await;

    // Generate and print the report
    println!("\n{}", results.generate_report());

    // Assert all tests passed
    assert_eq!(
        results.failure_count,
        0,
        "Integration tests failed: {} out of {} tests failed",
        results.failure_count,
        results.results.len()
    );
}

#[tokio::test]
async fn test_file_operations_memory_efficiency() {
    let mut harness = FileOperationsTestHarness::new().expect("Failed to create test harness");

    // Add specific memory efficiency scenarios
    harness.add_scenario(TestScenario {
        name: "extreme_memory_test".to_string(),
        description: "Test with 1GB file to ensure memory stays under limit".to_string(),
        test_type: TestType::MemoryEfficiency,
        expected_outcome: ExpectedOutcome::PerformanceWithinLimits {
            max_duration: std::time::Duration::from_secs(120),
            max_memory_mb: 500,
        },
        test_data: TestData {
            file_size: 1024 * 1024 * 1024, // 1GB
            algorithms: vec![anidb_client_core::HashAlgorithm::ED2K],
            content_type: ContentType::Random,
            concurrent_files: None,
        },
    });

    let results = harness.run_all_scenarios().await;

    // Check that memory limits were respected
    for result in &results.results {
        if let Some(metrics) = &result.performance_metrics
            && let Some(memory_mb) = metrics.memory_usage_mb
        {
            assert!(
                memory_mb <= 500,
                "Memory usage {} MB exceeded limit of 500 MB for scenario: {}",
                memory_mb,
                result.scenario_name
            );
        }
    }

    assert_eq!(results.failure_count, 0, "Memory efficiency tests failed");
}

#[tokio::test]
async fn test_file_operations_concurrent_processing() {
    let mut harness = FileOperationsTestHarness::new().expect("Failed to create test harness");

    // Add concurrent processing scenarios with different file counts
    for concurrent_count in [2, 5, 10] {
        harness.add_scenario(TestScenario {
            name: format!("concurrent_{concurrent_count}_files"),
            description: format!("Process {concurrent_count} files concurrently"),
            test_type: TestType::ConcurrentProcessing,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 5 * 1024 * 1024, // 5MB each
                algorithms: vec![
                    anidb_client_core::HashAlgorithm::ED2K,
                    anidb_client_core::HashAlgorithm::CRC32,
                ],
                content_type: ContentType::Random,
                concurrent_files: Some(concurrent_count),
            },
        });
    }

    let results = harness.run_all_scenarios().await;

    assert_eq!(
        results.failure_count, 0,
        "Concurrent processing tests failed"
    );
}

#[tokio::test]
async fn test_file_operations_progress_reporting() {
    let mut harness = FileOperationsTestHarness::new().expect("Failed to create test harness");

    // Test progress reporting with various file sizes
    for (size_mb, name) in [(1, "small"), (10, "medium"), (50, "large")] {
        harness.add_scenario(TestScenario {
            name: format!("progress_{name}_file"),
            description: format!("Verify progress reporting for {size_mb}MB file"),
            test_type: TestType::ProgressReporting,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: size_mb * 1024 * 1024,
                algorithms: vec![anidb_client_core::HashAlgorithm::ED2K],
                content_type: ContentType::Deterministic(42),
                concurrent_files: None,
            },
        });
    }

    let results = harness.run_all_scenarios().await;

    assert_eq!(results.failure_count, 0, "Progress reporting tests failed");
}

#[tokio::test]
async fn test_file_operations_error_handling() {
    let mut harness = FileOperationsTestHarness::new().expect("Failed to create test harness");

    // Add various error scenarios
    harness.add_scenario(TestScenario {
        name: "handle_corrupted_file".to_string(),
        description: "Process corrupted file without panicking".to_string(),
        test_type: TestType::ErrorHandling,
        expected_outcome: ExpectedOutcome::Success,
        test_data: TestData {
            file_size: 1024,
            algorithms: vec![anidb_client_core::HashAlgorithm::ED2K],
            content_type: ContentType::Corrupted,
            concurrent_files: None,
        },
    });

    harness.add_scenario(TestScenario {
        name: "handle_empty_file".to_string(),
        description: "Process empty file correctly".to_string(),
        test_type: TestType::FileValidation,
        expected_outcome: ExpectedOutcome::Success,
        test_data: TestData {
            file_size: 0,
            algorithms: vec![
                anidb_client_core::HashAlgorithm::ED2K,
                anidb_client_core::HashAlgorithm::CRC32,
            ],
            content_type: ContentType::Empty,
            concurrent_files: None,
        },
    });

    let results = harness.run_all_scenarios().await;

    assert_eq!(results.failure_count, 0, "Error handling tests failed");
}
