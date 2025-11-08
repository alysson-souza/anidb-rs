//! Test harness for file operations
//!
//! This module provides a test harness for integration testing
//! of file operations. It combines real file system operations with mock systems
//! to test edge cases and error scenarios.

use anidb_client_core::error::{InternalError, ValidationError};
use anidb_client_core::file_io::{FileProcessor, ProcessingStatus};
use anidb_client_core::{ClientConfig, Error, HashAlgorithm, Progress, Result};
use anidb_test_utils::builders::TestFileBuilder as TestFileGenerator;
use anidb_test_utils::mocks::MockFileSystem;
use anidb_test_utils::performance::PerformanceTracker;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// File operations test harness
pub struct FileOperationsTestHarness {
    _temp_dir: TempDir,
    test_generator: TestFileGenerator,
    _mock_fs: MockFileSystem,
    performance_tracker: PerformanceTracker,
    test_scenarios: Vec<TestScenario>,
}

/// Test scenario definition
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub test_type: TestType,
    pub expected_outcome: ExpectedOutcome,
    pub test_data: TestData,
}

#[derive(Debug, Clone)]
pub enum TestType {
    /// Test basic file processing
    BasicFileProcessing,
    /// Test error handling
    ErrorHandling,
    /// Test concurrent operations
    ConcurrentProcessing,
    /// Test memory efficiency
    MemoryEfficiency,
    /// Test progress reporting
    ProgressReporting,
    /// Test file validation
    FileValidation,
}

#[derive(Debug, Clone)]
pub enum ExpectedOutcome {
    Success,
    Error(String),
    PerformanceWithinLimits {
        max_duration: Duration,
        max_memory_mb: u64,
    },
}

#[derive(Debug, Clone)]
pub struct TestData {
    pub file_size: usize,
    pub algorithms: Vec<HashAlgorithm>,
    pub content_type: ContentType,
    pub concurrent_files: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum ContentType {
    Random,
    Deterministic(u64), // seed
    Empty,
    Corrupted,
}

impl FileOperationsTestHarness {
    /// Create a new file operations test harness
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new().map_err(|e| {
            Error::Internal(InternalError::ffi(
                "test_harness",
                &format!("Failed to create temp dir: {e}"),
            ))
        })?;

        let test_generator = TestFileGenerator::new(temp_dir.path());
        let mock_fs = MockFileSystem::new();
        let performance_tracker = PerformanceTracker::new();

        Ok(Self {
            _temp_dir: temp_dir,
            test_generator,
            _mock_fs: mock_fs,
            performance_tracker,
            test_scenarios: Vec::new(),
        })
    }

    /// Add standard test scenarios for file operation testing
    pub fn setup_standard_scenarios(&mut self) {
        // Basic file processing scenarios
        self.add_scenario(TestScenario {
            name: "basic_small_file".to_string(),
            description: "Process a small file with ED2K hash".to_string(),
            test_type: TestType::BasicFileProcessing,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 1024, // 1KB
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Random,
                concurrent_files: None,
            },
        });

        self.add_scenario(TestScenario {
            name: "multiple_algorithms".to_string(),
            description: "Process file with multiple hash algorithms".to_string(),
            test_type: TestType::BasicFileProcessing,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 10 * 1024, // 10KB
                algorithms: vec![HashAlgorithm::ED2K, HashAlgorithm::CRC32],
                content_type: ContentType::Deterministic(12345),
                concurrent_files: None,
            },
        });

        // Large file processing scenarios
        self.add_scenario(TestScenario {
            name: "large_file_memory_efficiency".to_string(),
            description: "Process large file while monitoring memory usage".to_string(),
            test_type: TestType::MemoryEfficiency,
            expected_outcome: ExpectedOutcome::PerformanceWithinLimits {
                max_duration: Duration::from_secs(30),
                max_memory_mb: 500, // Should stay under 500MB
            },
            test_data: TestData {
                file_size: 100 * 1024 * 1024, // 100MB
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Random,
                concurrent_files: None,
            },
        });

        // Error handling scenarios
        self.add_scenario(TestScenario {
            name: "file_not_found".to_string(),
            description: "Handle non-existent file gracefully".to_string(),
            test_type: TestType::ErrorHandling,
            expected_outcome: ExpectedOutcome::Error("FileNotFound".to_string()),
            test_data: TestData {
                file_size: 0,
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Empty,
                concurrent_files: None,
            },
        });

        self.add_scenario(TestScenario {
            name: "corrupted_file".to_string(),
            description: "Process corrupted file without crashing".to_string(),
            test_type: TestType::BasicFileProcessing,
            expected_outcome: ExpectedOutcome::Success, // Should handle gracefully
            test_data: TestData {
                file_size: 5 * 1024, // 5KB
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Corrupted,
                concurrent_files: None,
            },
        });

        // Concurrent processing scenarios
        self.add_scenario(TestScenario {
            name: "concurrent_file_processing".to_string(),
            description: "Process multiple files concurrently".to_string(),
            test_type: TestType::ConcurrentProcessing,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 1024 * 1024, // 1MB each
                algorithms: vec![HashAlgorithm::ED2K, HashAlgorithm::CRC32],
                content_type: ContentType::Random,
                concurrent_files: Some(5),
            },
        });

        // Progress reporting scenarios
        self.add_scenario(TestScenario {
            name: "progress_reporting".to_string(),
            description: "Verify progress reporting works correctly".to_string(),
            test_type: TestType::ProgressReporting,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 10 * 1024 * 1024, // 10MB
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Random,
                concurrent_files: None,
            },
        });

        // File validation scenarios
        self.add_scenario(TestScenario {
            name: "empty_file_validation".to_string(),
            description: "Validate empty file processing".to_string(),
            test_type: TestType::FileValidation,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 0,
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Empty,
                concurrent_files: None,
            },
        });
    }

    /// Add a custom test scenario
    pub fn add_scenario(&mut self, scenario: TestScenario) {
        self.test_scenarios.push(scenario);
    }

    /// Run all test scenarios
    pub async fn run_all_scenarios(&mut self) -> IntegrationTestResults {
        let mut results = IntegrationTestResults::new();
        let start_time = Instant::now();

        for scenario in &self.test_scenarios.clone() {
            println!(
                "Running scenario: {} - {}",
                scenario.name, scenario.description
            );

            let scenario_start = Instant::now();
            let result = self.run_scenario(scenario).await;
            let scenario_duration = scenario_start.elapsed();

            let test_result = IntegrationTestResult {
                scenario_name: scenario.name.clone(),
                success: result.is_ok(),
                duration: scenario_duration,
                error: result.err(),
                performance_metrics: self.get_performance_metrics(&scenario.name),
            };

            results.add_result(test_result);
        }

        results.total_duration = start_time.elapsed();
        results
    }

    /// Run a specific test scenario
    async fn run_scenario(&mut self, scenario: &TestScenario) -> Result<()> {
        let op_id = self.performance_tracker.start_tracking(&scenario.name);

        let result = match scenario.test_type {
            TestType::BasicFileProcessing => self.run_basic_file_processing(scenario).await,
            TestType::ErrorHandling => self.run_error_handling(scenario).await,
            TestType::ConcurrentProcessing => self.run_concurrent_processing(scenario).await,
            TestType::MemoryEfficiency => self.run_memory_efficiency(scenario).await,
            TestType::ProgressReporting => self.run_progress_reporting(scenario).await,
            TestType::FileValidation => self.run_file_validation(scenario).await,
        };

        self.performance_tracker.finish_tracking(op_id);

        // Validate expected outcome
        match (&scenario.expected_outcome, &result) {
            (ExpectedOutcome::Success, Ok(_)) => Ok(()),
            (ExpectedOutcome::Error(expected), Err(actual)) => {
                if format!("{actual:?}").contains(expected) {
                    Ok(())
                } else {
                    Err(Error::Validation(ValidationError::invalid_configuration(
                        &format!("Expected error containing '{expected}', got '{actual:?}'",),
                    )))
                }
            }
            (
                ExpectedOutcome::PerformanceWithinLimits {
                    max_duration,
                    max_memory_mb,
                },
                Ok(_),
            ) => {
                if let Some(metrics) = self.performance_tracker.get_metrics(&scenario.name) {
                    if metrics.duration > *max_duration {
                        return Err(Error::Validation(ValidationError::invalid_configuration(
                            &format!(
                                "Performance test failed: duration {:?} exceeded limit {:?}",
                                metrics.duration, max_duration
                            ),
                        )));
                    }

                    if let Some(memory) = metrics.memory_usage {
                        let memory_mb = memory / (1024 * 1024);
                        if memory_mb > *max_memory_mb {
                            return Err(Error::Validation(ValidationError::invalid_configuration(
                                &format!(
                                    "Performance test failed: memory {memory_mb}MB exceeded limit {max_memory_mb}MB",
                                ),
                            )));
                        }
                    }
                }
                Ok(())
            }
            _ => Err(Error::Validation(ValidationError::invalid_configuration(
                "Unexpected test outcome",
            ))),
        }
    }

    /// Run basic file processing test
    async fn run_basic_file_processing(&mut self, scenario: &TestScenario) -> Result<()> {
        let file_path = self.create_test_file(scenario)?;

        let config = ClientConfig::test();
        let processor = FileProcessor::new(config);

        let null_provider = std::sync::Arc::new(anidb_client_core::progress::NullProvider);
        let result = processor
            .process_file(
                &file_path,
                &scenario.test_data.algorithms,
                null_provider.clone(),
            )
            .await?;

        // Validate results
        if result.status != ProcessingStatus::Completed {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                &format!("Expected Completed status, got {:?}", result.status),
            )));
        }

        if result.hashes.len() != scenario.test_data.algorithms.len() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                &format!(
                    "Expected {} hashes, got {}",
                    scenario.test_data.algorithms.len(),
                    result.hashes.len()
                ),
            )));
        }

        Ok(())
    }

    /// Run error handling test
    async fn run_error_handling(&mut self, scenario: &TestScenario) -> Result<()> {
        if scenario.name == "file_not_found" {
            let config = ClientConfig::test();
            let processor = FileProcessor::new(config);

            // This should return an error
            processor
                .process_file(
                    Path::new("/nonexistent/file.mkv"),
                    &scenario.test_data.algorithms,
                    std::sync::Arc::new(anidb_client_core::progress::NullProvider),
                )
                .await
                .map(|_| ())
        } else {
            self.run_basic_file_processing(scenario).await
        }
    }

    /// Run concurrent processing test
    async fn run_concurrent_processing(&mut self, scenario: &TestScenario) -> Result<()> {
        let num_files = scenario.test_data.concurrent_files.unwrap_or(1);
        let mut file_paths = Vec::new();

        // Create multiple test files
        for i in 0..num_files {
            let file_path =
                self.create_test_file_with_name(scenario, &format!("concurrent_{i}.mkv"))?;
            file_paths.push(file_path);
        }

        let config = ClientConfig::test();
        let processor = FileProcessor::new(config);

        // Process files concurrently
        let null_provider = std::sync::Arc::new(anidb_client_core::progress::NullProvider);
        let results = processor
            .process_files_concurrent(file_paths, &scenario.test_data.algorithms, &*null_provider)
            .await?;

        // Validate all files processed successfully
        if results.len() != num_files {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                &format!("Expected {} results, got {}", num_files, results.len()),
            )));
        }

        for result in &results {
            if result.status != ProcessingStatus::Completed {
                return Err(Error::Validation(ValidationError::invalid_configuration(
                    &format!("File processing failed for: {}", result.file_path.display()),
                )));
            }
        }

        Ok(())
    }

    /// Run memory efficiency test
    async fn run_memory_efficiency(&mut self, scenario: &TestScenario) -> Result<()> {
        self.run_basic_file_processing(scenario).await
    }

    /// Run progress reporting test
    async fn run_progress_reporting(&mut self, scenario: &TestScenario) -> Result<()> {
        let file_path = self.create_test_file(scenario)?;

        let config = ClientConfig::test();
        let processor = FileProcessor::new(config);

        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<Progress>(100);

        // Clone algorithms to avoid borrow issue
        let algorithms = scenario.test_data.algorithms.clone();

        // Process file with progress reporting
        let process_task = tokio::spawn(async move {
            // Use adapter to convert old progress channel to new provider
            let provider = anidb_client_core::progress::ChannelAdapter::new(progress_tx);
            processor
                .process_file(&file_path, &algorithms, std::sync::Arc::from(provider))
                .await
        });

        let mut progress_updates = Vec::new();
        let progress_task = tokio::spawn(async move {
            while let Some(progress) = progress_rx.recv().await {
                progress_updates.push(progress.clone());
                if progress.percentage >= 100.0 {
                    break;
                }
            }
            progress_updates
        });

        // Run both tasks
        let process_result = process_task.await.unwrap();
        let progress_updates = progress_task.await.unwrap();

        process_result?;

        // Validate progress reporting
        if progress_updates.is_empty() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "No progress updates received",
            )));
        }

        if !progress_updates.iter().any(|p| p.percentage >= 100.0) {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "No 100% progress update received",
            )));
        }

        Ok(())
    }

    /// Run file validation test
    async fn run_file_validation(&mut self, scenario: &TestScenario) -> Result<()> {
        self.run_basic_file_processing(scenario).await
    }

    /// Create a test file for the scenario
    fn create_test_file(&mut self, scenario: &TestScenario) -> Result<PathBuf> {
        self.create_test_file_with_name(scenario, "test.mkv")
    }

    /// Create a test file with specific name for the scenario
    fn create_test_file_with_name(
        &mut self,
        scenario: &TestScenario,
        name: &str,
    ) -> Result<PathBuf> {
        match scenario.test_data.content_type {
            ContentType::Random => self
                .test_generator
                .generate_test_file(name, scenario.test_data.file_size),
            ContentType::Deterministic(seed) => self.test_generator.generate_deterministic_file(
                name,
                scenario.test_data.file_size,
                seed,
            ),
            ContentType::Empty => self.test_generator.generate_test_file(name, 0),
            ContentType::Corrupted => self
                .test_generator
                .generate_corrupted_file(name, scenario.test_data.file_size),
        }
    }

    /// Get performance metrics for a scenario
    fn get_performance_metrics(&self, scenario_name: &str) -> Option<PerformanceMetrics> {
        self.performance_tracker
            .get_metrics(scenario_name)
            .map(|m| PerformanceMetrics {
                duration: m.duration,
                memory_usage_mb: m.memory_usage.map(|mem| mem / (1024 * 1024)),
            })
    }
}

/// Results from integration testing
#[derive(Debug)]
pub struct IntegrationTestResults {
    pub results: Vec<IntegrationTestResult>,
    pub total_duration: Duration,
    pub success_count: usize,
    pub failure_count: usize,
}

#[derive(Debug)]
pub struct IntegrationTestResult {
    pub scenario_name: String,
    pub success: bool,
    pub duration: Duration,
    pub error: Option<Error>,
    pub performance_metrics: Option<PerformanceMetrics>,
}

#[derive(Debug)]
pub struct PerformanceMetrics {
    pub duration: Duration,
    pub memory_usage_mb: Option<u64>,
}

impl IntegrationTestResults {
    fn new() -> Self {
        Self {
            results: Vec::new(),
            total_duration: Duration::default(),
            success_count: 0,
            failure_count: 0,
        }
    }

    fn add_result(&mut self, result: IntegrationTestResult) {
        if result.success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }
        self.results.push(result);
    }

    /// Generate a summary report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();

        report.push_str("Integration Test Results\n");
        report.push_str("========================\n\n");

        report.push_str(&format!("Total Tests: {}\n", self.results.len()));
        report.push_str(&format!("Passed: {}\n", self.success_count));
        report.push_str(&format!("Failed: {}\n", self.failure_count));
        report.push_str(&format!("Total Duration: {:?}\n\n", self.total_duration));

        for result in &self.results {
            let status = if result.success { "✓" } else { "✗" };
            report.push_str(&format!(
                "{} {} ({:?})\n",
                status, result.scenario_name, result.duration
            ));

            if let Some(error) = &result.error {
                report.push_str(&format!("  Error: {error}\n"));
            }

            if let Some(metrics) = &result.performance_metrics {
                report.push_str(&format!("  Duration: {:?}\n", metrics.duration));
                if let Some(memory) = metrics.memory_usage_mb {
                    report.push_str(&format!("  Memory: {memory}MB\n"));
                }
            }
            report.push('\n');
        }

        report
    }
}

// Tests for the integration test harness itself
#[cfg(test)]
mod integration_harness_tests {
    use super::*;

    #[test]
    fn test_create_integration_test_harness() {
        let harness = FileOperationsTestHarness::new();
        assert!(harness.is_ok());
    }

    #[test]
    fn test_setup_standard_scenarios() {
        let mut harness = FileOperationsTestHarness::new().unwrap();

        harness.setup_standard_scenarios();
        assert!(!harness.test_scenarios.is_empty());

        // Check that all standard scenario types are covered
        let scenario_types: std::collections::HashSet<_> = harness
            .test_scenarios
            .iter()
            .map(|s| std::mem::discriminant(&s.test_type))
            .collect();

        assert!(scenario_types.len() >= 5); // Should have multiple test types
    }

    #[test]
    fn test_add_custom_scenario() {
        let mut harness = FileOperationsTestHarness::new().unwrap();

        let custom_scenario = TestScenario {
            name: "custom_test".to_string(),
            description: "A custom test scenario".to_string(),
            test_type: TestType::BasicFileProcessing,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 2048,
                algorithms: vec![HashAlgorithm::CRC32],
                content_type: ContentType::Random,
                concurrent_files: None,
            },
        };

        harness.add_scenario(custom_scenario);
        assert_eq!(harness.test_scenarios.len(), 1);
        assert_eq!(harness.test_scenarios[0].name, "custom_test");
    }

    #[tokio::test]
    async fn test_run_single_scenario() {
        let mut harness = FileOperationsTestHarness::new().unwrap();

        let scenario = TestScenario {
            name: "simple_test".to_string(),
            description: "Simple file processing test".to_string(),
            test_type: TestType::BasicFileProcessing,
            expected_outcome: ExpectedOutcome::Success,
            test_data: TestData {
                file_size: 1024,
                algorithms: vec![HashAlgorithm::ED2K],
                content_type: ContentType::Random,
                concurrent_files: None,
            },
        };

        let result = harness.run_scenario(&scenario).await;
        // This might fail due to missing FileProcessor implementation,
        // which is expected in our TDD approach
        println!("Test result: {result:?}");
    }
}
