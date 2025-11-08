//! Mock implementation of AniDBClient for testing

use anidb_client_core::api::{
    AnimeIdentification, BatchOptions, BatchResult, FileResult, IdentificationSource,
    ProcessOptions,
};
use anidb_client_core::file_io::ProcessingStatus;
use anidb_client_core::{
    Error, HashAlgorithm, Result,
    error::{InternalError, IoError, ProtocolError, ValidationError},
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock implementation of AniDBClient for testing
///
/// This mock provides configurable behavior for all AniDBClient operations,
/// allowing dependent teams to test their components without requiring
/// actual files or network connectivity.
///
/// # Examples
///
/// ```rust,no_run
/// use anidb_test_utils::MockAniDBClient;
/// use anidb_client_core::api::ProcessOptions;
/// use anidb_client_core::HashAlgorithm;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut mock = MockAniDBClient::new();
///
/// // Configure to return success for file processing
/// mock.expect_file_processing_success();
///
/// let options = ProcessOptions::new()
///     .with_algorithms(&[HashAlgorithm::ED2K]);
///
/// let result = mock.process_file(Path::new("test.mkv"), options).await;
/// assert!(result.is_ok());
/// # Ok(())
/// # }
/// ```
pub struct MockAniDBClient {
    behavior: Arc<Mutex<MockBehavior>>,
}

/// Configuration for mock behavior
#[derive(Debug, Clone)]
struct MockBehavior {
    file_processing_result: std::result::Result<MockFileResult, MockError>,
    anime_identification_result: std::result::Result<MockAnimeInfo, MockError>,
    processing_delay: Duration,
    should_send_progress: bool,
    batch_behavior: BatchBehavior,
}

#[derive(Debug, Clone)]
enum MockError {
    FileNotFound(String),
    NetworkOffline,
    InvalidConfiguration(String),
    Custom(String),
}

impl From<MockError> for Error {
    fn from(mock_error: MockError) -> Self {
        match mock_error {
            MockError::FileNotFound(path) => {
                Error::Io(IoError::file_not_found(&PathBuf::from(path)))
            }
            MockError::NetworkOffline => Error::Protocol(ProtocolError::NetworkOffline),
            MockError::InvalidConfiguration(msg) => {
                Error::Validation(ValidationError::invalid_configuration(&msg))
            }
            MockError::Custom(msg) => Error::Internal(InternalError::ffi("mock_error", &msg)),
        }
    }
}

#[derive(Debug, Clone)]
struct MockFileResult {
    status: ProcessingStatus,
    hashes: HashMap<HashAlgorithm, String>,
}

#[derive(Debug, Clone)]
struct MockAnimeInfo {
    anime_id: u64,
    title: String,
    episode_number: u32,
}

#[derive(Debug, Clone)]
enum BatchBehavior {
    AllSuccess,
    AllFailure,
    PartialSuccess { success_count: usize },
    CustomResults(Vec<std::result::Result<MockFileResult, MockError>>),
}

impl Default for MockBehavior {
    fn default() -> Self {
        let mut hashes = HashMap::new();
        hashes.insert(
            HashAlgorithm::ED2K,
            "098f6bcd4621d373cade4e832627b4f6".to_string(),
        );
        hashes.insert(HashAlgorithm::CRC32, "d87f7e0c".to_string());

        Self {
            file_processing_result: Ok(MockFileResult {
                status: ProcessingStatus::Completed,
                hashes,
            }),
            anime_identification_result: Ok(MockAnimeInfo {
                anime_id: 1,
                title: "Mock Anime".to_string(),
                episode_number: 1,
            }),
            processing_delay: Duration::from_millis(10),
            should_send_progress: true,
            batch_behavior: BatchBehavior::AllSuccess,
        }
    }
}

impl MockAniDBClient {
    /// Create a new mock client with default behavior
    pub fn new() -> Self {
        Self {
            behavior: Arc::new(Mutex::new(MockBehavior::default())),
        }
    }

    /// Configure the mock to return successful file processing
    pub fn expect_file_processing_success(&mut self) {
        let mut behavior = self.behavior.lock().unwrap();
        let mut hashes = HashMap::new();
        hashes.insert(
            HashAlgorithm::ED2K,
            "098f6bcd4621d373cade4e832627b4f6".to_string(),
        );
        hashes.insert(HashAlgorithm::CRC32, "d87f7e0c".to_string());

        behavior.file_processing_result = Ok(MockFileResult {
            status: ProcessingStatus::Completed,
            hashes,
        });
    }

    /// Configure the mock to return a file processing error
    pub fn expect_file_processing_error(&mut self, error: Error) {
        use anidb_client_core::error::{IoErrorKind, ProtocolError, ValidationError};

        let mut behavior = self.behavior.lock().unwrap();
        let mock_error = match error {
            Error::Io(ref io_err) if io_err.kind == IoErrorKind::FileNotFound => {
                MockError::FileNotFound(
                    io_err
                        .path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
            }
            Error::Protocol(ProtocolError::NetworkOffline) => MockError::NetworkOffline,
            Error::Validation(ValidationError::InvalidConfiguration { ref message }) => {
                MockError::InvalidConfiguration(message.clone())
            }
            _ => MockError::Custom(format!("{error}")),
        };
        behavior.file_processing_result = Err(mock_error);
    }

    /// Configure the mock to return specific anime identification
    pub fn expect_anime_identification(&mut self, title: &str, episode_number: u32) {
        let mut behavior = self.behavior.lock().unwrap();
        behavior.anime_identification_result = Ok(MockAnimeInfo {
            anime_id: 12345,
            title: title.to_string(),
            episode_number,
        });
    }

    /// Configure the mock to return an identification error
    pub fn expect_identification_error(&mut self, error: Error) {
        use anidb_client_core::error::{IoErrorKind, ProtocolError, ValidationError};

        let mut behavior = self.behavior.lock().unwrap();
        let mock_error = match error {
            Error::Io(ref io_err) if io_err.kind == IoErrorKind::FileNotFound => {
                MockError::FileNotFound(
                    io_err
                        .path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
            }
            Error::Protocol(ProtocolError::NetworkOffline) => MockError::NetworkOffline,
            Error::Validation(ValidationError::InvalidConfiguration { ref message }) => {
                MockError::InvalidConfiguration(message.clone())
            }
            _ => MockError::Custom(format!("{error}")),
        };
        behavior.anime_identification_result = Err(mock_error);
    }

    /// Set the processing delay for operations
    pub fn set_processing_delay(&mut self, delay: Duration) {
        let mut behavior = self.behavior.lock().unwrap();
        behavior.processing_delay = delay;
    }

    /// Enable or disable progress reporting
    pub fn set_progress_reporting(&mut self, enabled: bool) {
        let mut behavior = self.behavior.lock().unwrap();
        behavior.should_send_progress = enabled;
    }

    /// Configure batch processing to succeed for all files
    pub fn expect_batch_all_success(&mut self) {
        let mut behavior = self.behavior.lock().unwrap();
        behavior.batch_behavior = BatchBehavior::AllSuccess;
    }

    /// Configure batch processing to fail for all files
    pub fn expect_batch_all_failure(&mut self) {
        let mut behavior = self.behavior.lock().unwrap();
        behavior.batch_behavior = BatchBehavior::AllFailure;
    }

    /// Configure batch processing for partial success
    pub fn expect_batch_partial_success(&mut self, success_count: usize) {
        let mut behavior = self.behavior.lock().unwrap();
        behavior.batch_behavior = BatchBehavior::PartialSuccess { success_count };
    }

    /// Configure batch processing with a custom list of results
    pub fn expect_batch_custom_results(
        &mut self,
        results: Vec<std::result::Result<FileResult, Error>>,
    ) {
        let mut behavior = self.behavior.lock().unwrap();
        let mock_results = results
            .into_iter()
            .map(|res| match res {
                Ok(file_result) => Ok(MockFileResult {
                    status: file_result.status,
                    hashes: file_result.hashes,
                }),
                Err(e) => {
                    use anidb_client_core::error::{IoErrorKind, ProtocolError, ValidationError};

                    Err(match e {
                        Error::Io(ref io_err) if io_err.kind == IoErrorKind::FileNotFound => {
                            MockError::FileNotFound(
                                io_err
                                    .path
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default(),
                            )
                        }
                        Error::Protocol(ProtocolError::NetworkOffline) => MockError::NetworkOffline,
                        Error::Validation(ValidationError::InvalidConfiguration {
                            ref message,
                        }) => MockError::InvalidConfiguration(message.clone()),
                        _ => MockError::Custom(format!("{e}")),
                    })
                }
            })
            .collect();
        behavior.batch_behavior = BatchBehavior::CustomResults(mock_results);
    }
    /// Check if the mock client is ready (always true for mocks)
    pub fn is_ready(&self) -> bool {
        true
    }

    /// Mock implementation of file processing
    pub async fn process_file(
        &self,
        file_path: &Path,
        options: ProcessOptions,
    ) -> Result<FileResult> {
        let behavior = self.behavior.lock().unwrap().clone();

        // Validate options (same as real implementation)
        options.validate()?;

        // Simulate processing delay
        if !behavior.processing_delay.is_zero() {
            tokio::time::sleep(behavior.processing_delay).await;
        }

        // Send progress updates if configured and provider is available
        if behavior.should_send_progress
            && let Some(provider) = options.progress_provider()
        {
            let file_size = 1024 * 1024; // Mock 1MB file
            let steps = 5;

            for i in 0..=steps {
                let bytes_processed = (file_size as f64 * (i as f64 / steps as f64)) as u64;

                provider.report(anidb_client_core::progress::ProgressUpdate::FileProgress {
                    path: file_path.to_path_buf(),
                    bytes_processed,
                    total_bytes: file_size,
                    operation: format!("Mock processing step {}", i + 1),
                    throughput_mbps: Some(100.0), // Mock 100 MB/s
                    memory_usage_bytes: Some(10 * 1024 * 1024), // Mock 10MB usage
                    buffer_size: Some(8 * 1024 * 1024), // Mock 8MB buffer
                });

                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }

        // Return configured result
        match behavior.file_processing_result {
            Ok(mock_result) => {
                // Filter hashes based on requested algorithms
                let mut filtered_hashes = HashMap::new();
                for algorithm in options.algorithms() {
                    if let Some(hash) = mock_result.hashes.get(algorithm) {
                        filtered_hashes.insert(*algorithm, hash.clone());
                    }
                }

                Ok(FileResult {
                    file_path: file_path.to_path_buf(),
                    file_size: 1024 * 1024, // Mock 1MB file
                    hashes: filtered_hashes,
                    status: mock_result.status,
                    processing_time: behavior.processing_delay,
                    anime_info: behavior.anime_identification_result.ok().map(|info| {
                        AnimeIdentification {
                            anime_id: info.anime_id,
                            episode_id: info.anime_id + 1000,
                            title: info.title,
                            episode_number: info.episode_number,
                            source: IdentificationSource::AniDB,
                        }
                    }),
                })
            }
            Err(mock_error) => Err(mock_error.into()),
        }
    }

    /// Mock implementation of batch processing
    pub async fn process_batch(
        &self,
        file_paths: &[PathBuf],
        options: BatchOptions,
    ) -> Result<BatchResult> {
        let behavior = self.behavior.lock().unwrap().clone();

        // Validate options
        options.validate()?;

        let mut results = Vec::new();
        let total_files = file_paths.len();

        match behavior.batch_behavior {
            BatchBehavior::AllSuccess => {
                for file_path in file_paths {
                    let process_options =
                        ProcessOptions::new().with_algorithms(options.algorithms());

                    let result = self.process_file(file_path, process_options).await;
                    results.push(result);
                }
            }
            BatchBehavior::AllFailure => {
                for _ in file_paths {
                    results.push(Err(Error::Io(IoError::file_not_found(&PathBuf::from(
                        "mock_error.mkv",
                    )))));
                }
            }
            BatchBehavior::PartialSuccess { success_count } => {
                let success_count = success_count.min(total_files);

                for (i, file_path) in file_paths.iter().enumerate() {
                    if i < success_count {
                        let process_options =
                            ProcessOptions::new().with_algorithms(options.algorithms());

                        let result = self.process_file(file_path, process_options).await;
                        results.push(result);
                    } else {
                        results.push(Err(Error::Io(IoError::file_not_found(file_path))));
                    }
                }
            }
            BatchBehavior::CustomResults(custom_results) => {
                for (i, file_path) in file_paths.iter().enumerate() {
                    if let Some(mock_result) = custom_results.get(i) {
                        let result = match mock_result {
                            Ok(mock_file_result) => Ok(FileResult {
                                file_path: file_path.clone(),
                                file_size: 1024 * 1024,
                                hashes: mock_file_result.hashes.clone(),
                                status: mock_file_result.status.clone(),
                                processing_time: behavior.processing_delay,
                                anime_info: None,
                            }),
                            Err(mock_error) => Err(mock_error.clone().into()),
                        };
                        results.push(result);
                    } else {
                        results.push(Err(Error::Validation(
                            ValidationError::invalid_configuration("No mock result configured"),
                        )));
                    }
                }
            }
        }

        let batch_result = BatchResult::from_results(results);
        Ok(batch_result)
    }

    /// Mock implementation of anime identification
    pub async fn identify_file(&self, _hash: &str, _size: u64) -> Result<AnimeIdentification> {
        let behavior = self.behavior.lock().unwrap().clone();

        // Simulate network delay
        tokio::time::sleep(behavior.processing_delay).await;

        match behavior.anime_identification_result {
            Ok(mock_info) => Ok(AnimeIdentification {
                anime_id: mock_info.anime_id,
                episode_id: mock_info.anime_id + 1000,
                title: mock_info.title,
                episode_number: mock_info.episode_number,
                source: IdentificationSource::AniDB,
            }),
            Err(mock_error) => Err(mock_error.into()),
        }
    }
}

impl Default for MockAniDBClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock hash calculator for testing
///
/// Provides predictable hash calculations for testing purposes
/// without the overhead of actual hash computation.
pub struct MockHashCalculator {
    predefined_hashes: HashMap<(HashAlgorithm, Vec<u8>), String>,
}

impl MockHashCalculator {
    /// Create a new mock hash calculator
    pub fn new() -> Self {
        let mut predefined_hashes = HashMap::new();

        // Add some common test cases
        predefined_hashes.insert(
            (HashAlgorithm::ED2K, b"test content".to_vec()),
            "098f6bcd4621d373cade4e832627b4f6".to_string(),
        );
        predefined_hashes.insert(
            (HashAlgorithm::CRC32, b"test content".to_vec()),
            "d87f7e0c".to_string(),
        );

        Self { predefined_hashes }
    }

    /// Add a predefined hash result
    pub fn add_hash(&mut self, algorithm: HashAlgorithm, data: &[u8], expected_hash: &str) {
        self.predefined_hashes
            .insert((algorithm, data.to_vec()), expected_hash.to_string());
    }

    /// Calculate a mock hash
    pub fn calculate_hash(&self, algorithm: HashAlgorithm, data: &[u8]) -> String {
        self.predefined_hashes
            .get(&(algorithm, data.to_vec()))
            .cloned()
            .unwrap_or_else(|| {
                format!(
                    "mock_{}_{:08x}",
                    format!("{algorithm:?}").to_lowercase(),
                    data.len()
                )
            })
    }
}

impl Default for MockHashCalculator {
    fn default() -> Self {
        Self::new()
    }
}
