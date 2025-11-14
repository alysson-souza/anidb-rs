//! Enhanced test utilities with builder patterns
//!
//! This module provides improved builder patterns for test data generation,
//! scenario creation, and mock client configuration. It's designed to reduce
//! boilerplate in tests and make them more readable.

#![cfg(any(test, feature = "test-utils"))]

use crate::{Error, FileResult, HashAlgorithm, error::IoError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// TestDataBuilder - Enhanced with presets and auto-generation
// ============================================================================

/// Builder for creating test data with fluent API and presets
pub struct TestDataBuilder {
    anime_title: String,
    episode_number: u32,
    file_size: u64,
    file_path: Option<PathBuf>,
    expected_hashes: HashMap<HashAlgorithm, String>,
    content_pattern: ContentPattern,
    corrupted: bool,
}

/// Content patterns for deterministic test file generation
#[derive(Clone, Copy, Debug)]
pub enum ContentPattern {
    Zeros,
    Random(u64), // seed
    Repeating(u8),
    Gradient,
    RealVideo, // Simulates real video file patterns
}

impl TestDataBuilder {
    /// Create a new builder with defaults
    pub fn new() -> Self {
        Self {
            anime_title: String::from("Test Anime"),
            episode_number: 1,
            file_size: 1024 * 1024, // 1MB default
            file_path: None,
            expected_hashes: HashMap::new(),
            content_pattern: ContentPattern::Zeros,
            corrupted: false,
        }
    }
}

impl Default for TestDataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataBuilder {
    // Preset configurations

    /// Small file preset (< 1MB)
    pub fn small_file() -> Self {
        Self::new()
            .with_file_size(512 * 1024)
            .with_standard_hashes()
    }

    /// Large file preset (> 100MB)
    pub fn large_file() -> Self {
        Self::new()
            .with_file_size(150 * 1024 * 1024)
            .with_standard_hashes()
    }

    /// Anime episode preset with typical properties
    pub fn anime_episode(title: &str, episode: u32) -> Self {
        Self::new()
            .with_anime_title(title)
            .with_episode_number(episode)
            .with_file_size(350 * 1024 * 1024) // 350MB typical
            .with_standard_hashes()
            .with_content_pattern(ContentPattern::RealVideo)
    }

    /// ED2K boundary test file (exactly 9.5MB)
    pub fn ed2k_boundary_file() -> Self {
        Self::new()
            .with_file_size(9728000) // ED2K chunk size
            .with_standard_hashes()
    }

    /// Multi-chunk ED2K file
    pub fn ed2k_multichunk_file() -> Self {
        Self::new()
            .with_file_size(9728000 * 3 + 1024) // 3 chunks + extra
            .with_standard_hashes()
    }

    // Builder methods

    pub fn with_anime_title(mut self, title: &str) -> Self {
        self.anime_title = title.to_string();
        self
    }

    pub fn with_episode_number(mut self, episode: u32) -> Self {
        self.episode_number = episode;
        self
    }

    pub fn with_file_size(mut self, size: u64) -> Self {
        self.file_size = size;
        self
    }

    pub fn with_file_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.file_path = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_hash(mut self, algorithm: HashAlgorithm, hash: &str) -> Self {
        self.expected_hashes.insert(algorithm, hash.to_string());
        self
    }

    /// Add standard hashes (ED2K, CRC32, MD5, SHA1)
    pub fn with_standard_hashes(mut self) -> Self {
        // Auto-generate deterministic hashes based on file properties
        let seed = format!(
            "{}-{}-{}",
            self.anime_title, self.episode_number, self.file_size
        );

        self.expected_hashes.insert(
            HashAlgorithm::ED2K,
            Self::generate_deterministic_hash(&seed, "ed2k"),
        );
        self.expected_hashes.insert(
            HashAlgorithm::CRC32,
            Self::generate_deterministic_hash(&seed, "crc32"),
        );
        self.expected_hashes.insert(
            HashAlgorithm::MD5,
            Self::generate_deterministic_hash(&seed, "md5"),
        );
        self.expected_hashes.insert(
            HashAlgorithm::SHA1,
            Self::generate_deterministic_hash(&seed, "sha1"),
        );

        self
    }

    /// Use "auto" to generate deterministic hashes
    pub fn with_auto_hashes(self) -> Self {
        self.with_standard_hashes()
    }

    pub fn with_content_pattern(mut self, pattern: ContentPattern) -> Self {
        self.content_pattern = pattern;
        self
    }

    pub fn corrupted(mut self) -> Self {
        self.corrupted = true;
        self
    }

    /// Build the test data
    pub fn build(self) -> TestFileData {
        let file_path = self.file_path.unwrap_or_else(|| {
            PathBuf::from(format!(
                "/test/{}_ep{}.mkv",
                self.anime_title.replace(' ', "_"),
                self.episode_number
            ))
        });

        TestFileData {
            anime_title: self.anime_title,
            episode_number: self.episode_number,
            file_size: self.file_size,
            file_path,
            expected_hashes: self.expected_hashes,
            content_pattern: self.content_pattern,
            corrupted: self.corrupted,
        }
    }

    /// Generate deterministic hash for testing
    fn generate_deterministic_hash(seed: &str, algorithm: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        algorithm.hash(&mut hasher);
        let hash_value = hasher.finish();

        match algorithm {
            "ed2k" | "md5" => format!("{hash_value:032x}"),
            "sha1" => format!("{hash_value:040x}"),
            "crc32" => format!("{:08x}", hash_value as u32),
            _ => format!("{hash_value:x}"),
        }
    }
}

/// Test file data structure
#[derive(Debug, Clone)]
pub struct TestFileData {
    pub anime_title: String,
    pub episode_number: u32,
    pub file_size: u64,
    pub file_path: PathBuf,
    pub expected_hashes: HashMap<HashAlgorithm, String>,
    pub content_pattern: ContentPattern,
    pub corrupted: bool,
}

// ============================================================================
// ScenarioBuilder - Complex test scenarios
// ============================================================================

/// Builder for creating complex test scenarios
pub struct ScenarioBuilder {
    name: String,
    files: Vec<TestFileData>,
    errors: Vec<PlannedError>,
    delays: Vec<PlannedDelay>,
    network_conditions: NetworkConditions,
}

/// Planned error injection
#[derive(Debug, Clone)]
pub struct PlannedError {
    pub at_file_index: Option<usize>,
    pub at_time: Option<Duration>,
    pub error_type: ErrorType,
}

/// Error types for injection
#[derive(Debug, Clone)]
pub enum ErrorType {
    FileNotFound,
    PermissionDenied,
    NetworkTimeout,
    CorruptedData,
    DiskFull,
    Custom(String), // Store error message instead of Error
}

/// Planned delay injection
#[derive(Debug, Clone)]
pub struct PlannedDelay {
    pub at_file_index: Option<usize>,
    pub duration: Duration,
}

/// Network condition simulation
#[derive(Debug, Clone)]
pub struct NetworkConditions {
    pub latency: Duration,
    pub packet_loss: f32,
    pub offline: bool,
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self {
            latency: Duration::from_millis(50),
            packet_loss: 0.0,
            offline: false,
        }
    }
}

impl ScenarioBuilder {
    /// Create a new scenario builder
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            files: Vec::new(),
            errors: Vec::new(),
            delays: Vec::new(),
            network_conditions: NetworkConditions::default(),
        }
    }

    // Preset scenarios

    /// Batch processing scenario with multiple episodes
    pub fn batch_processing(anime_title: &str, episode_count: u32) -> Self {
        let mut builder = Self::new("batch_processing");

        for episode in 1..=episode_count {
            builder
                .files
                .push(TestDataBuilder::anime_episode(anime_title, episode).build());
        }

        builder
    }

    /// Error recovery scenario with planned failures
    pub fn error_recovery() -> Self {
        Self::new("error_recovery")
            .add_file(TestDataBuilder::small_file().build())
            .add_error_at_file(1, ErrorType::NetworkTimeout)
            .add_file(TestDataBuilder::small_file().build())
            .add_error_at_file(3, ErrorType::FileNotFound)
            .add_file(TestDataBuilder::small_file().build())
    }

    /// Performance test scenario with various file sizes
    pub fn performance_test() -> Self {
        Self::new("performance_test")
            .add_file(TestDataBuilder::new().with_file_size(1024).build()) // 1KB
            .add_file(TestDataBuilder::new().with_file_size(1024 * 1024).build()) // 1MB
            .add_file(
                TestDataBuilder::new()
                    .with_file_size(10 * 1024 * 1024)
                    .build(),
            ) // 10MB
            .add_file(
                TestDataBuilder::new()
                    .with_file_size(100 * 1024 * 1024)
                    .build(),
            ) // 100MB
            .add_file(
                TestDataBuilder::new()
                    .with_file_size(1024 * 1024 * 1024)
                    .build(),
            ) // 1GB
    }

    /// Network issues scenario
    pub fn network_issues() -> Self {
        Self::new("network_issues")
            .with_network_latency(Duration::from_secs(2))
            .with_packet_loss(0.1)
            .add_file(TestDataBuilder::small_file().build())
            .add_delay_at_file(0, Duration::from_secs(1))
            .add_file(TestDataBuilder::small_file().build())
    }

    /// Offline mode scenario
    pub fn offline_mode() -> Self {
        Self::new("offline_mode").offline().add_files(vec![
            TestDataBuilder::small_file().build(),
            TestDataBuilder::large_file().build(),
        ])
    }

    // Builder methods

    pub fn add_file(mut self, file: TestFileData) -> Self {
        self.files.push(file);
        self
    }

    pub fn add_files(mut self, files: Vec<TestFileData>) -> Self {
        self.files.extend(files);
        self
    }

    pub fn add_error_at_file(mut self, index: usize, error_type: ErrorType) -> Self {
        self.errors.push(PlannedError {
            at_file_index: Some(index),
            at_time: None,
            error_type,
        });
        self
    }

    pub fn add_error_at_time(mut self, time: Duration, error_type: ErrorType) -> Self {
        self.errors.push(PlannedError {
            at_file_index: None,
            at_time: Some(time),
            error_type,
        });
        self
    }

    pub fn add_delay_at_file(mut self, index: usize, duration: Duration) -> Self {
        self.delays.push(PlannedDelay {
            at_file_index: Some(index),
            duration,
        });
        self
    }

    pub fn with_network_latency(mut self, latency: Duration) -> Self {
        self.network_conditions.latency = latency;
        self
    }

    pub fn with_packet_loss(mut self, loss_rate: f32) -> Self {
        self.network_conditions.packet_loss = loss_rate.clamp(0.0, 1.0);
        self
    }

    pub fn offline(mut self) -> Self {
        self.network_conditions.offline = true;
        self
    }

    /// Build the scenario
    pub fn build(self) -> TestScenario {
        TestScenario {
            name: self.name,
            files: self.files,
            errors: self.errors,
            delays: self.delays,
            network_conditions: self.network_conditions,
        }
    }
}

/// Test scenario structure
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub files: Vec<TestFileData>,
    pub errors: Vec<PlannedError>,
    pub delays: Vec<PlannedDelay>,
    pub network_conditions: NetworkConditions,
}

// ============================================================================
// MockClientBuilder - Enhanced mock client configuration
// ============================================================================

/// Builder for creating mock AniDB clients with preset configurations
pub struct MockClientBuilder {
    responses: HashMap<String, MockResponse>,
    latency: Duration,
    failure_rate: f32,
    offline: bool,
    rate_limit: Option<Duration>,
}

/// Mock response configuration
#[derive(Debug, Clone)]
pub struct MockResponse {
    pub code: u16,
    pub data: String,
    pub delay: Option<Duration>,
}

impl MockClientBuilder {
    /// Create a new mock client builder
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            latency: Duration::from_millis(50),
            failure_rate: 0.0,
            offline: false,
            // Simulated AniDB rate limit between requests (safer 2.5s)
            rate_limit: Some(Duration::from_millis(2500)),
        }
    }
}

impl Default for MockClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl MockClientBuilder {
    /// Configure with standard AniDB responses
    pub fn with_standard_responses(mut self) -> Self {
        // Add common AniDB responses
        self.responses.insert(
            "AUTH".to_string(),
            MockResponse {
                code: 200,
                data: "200 LOGIN ACCEPTED".to_string(),
                delay: None,
            },
        );

        self.responses.insert(
            "FILE".to_string(),
            MockResponse {
                code: 220,
                data: "220 FILE\n1234567|12345|67890|Test Anime|01|720p".to_string(),
                delay: None,
            },
        );

        self.responses.insert(
            "LOGOUT".to_string(),
            MockResponse {
                code: 203,
                data: "203 LOGGED OUT".to_string(),
                delay: None,
            },
        );

        self
    }

    /// Configure with network issues
    pub fn with_network_issues(mut self) -> Self {
        self.latency = Duration::from_secs(2);
        self.failure_rate = 0.1; // 10% failure rate
        self
    }

    /// Configure for offline mode
    pub fn offline_mode(mut self) -> Self {
        self.offline = true;
        self
    }

    /// Add custom response
    pub fn with_response(mut self, command: &str, response: MockResponse) -> Self {
        self.responses.insert(command.to_string(), response);
        self
    }

    /// Set latency
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    /// Set failure rate (0.0 to 1.0)
    pub fn with_failure_rate(mut self, rate: f32) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Disable rate limiting (for testing)
    pub fn no_rate_limit(mut self) -> Self {
        self.rate_limit = None;
        self
    }

    /// Build the mock client
    pub fn build(self) -> MockAniDBClient {
        MockAniDBClient {
            responses: Arc::new(self.responses),
            latency: self.latency,
            failure_rate: self.failure_rate,
            offline: self.offline,
            rate_limit: self.rate_limit,
        }
    }
}

/// Mock AniDB client for testing
pub struct MockAniDBClient {
    responses: Arc<HashMap<String, MockResponse>>,
    latency: Duration,
    failure_rate: f32,
    offline: bool,
    rate_limit: Option<Duration>,
}

impl MockAniDBClient {
    /// Simulate sending a command
    pub async fn send_command(&self, command: &str) -> Result<MockResponse, Error> {
        // Simulate offline
        if self.offline {
            return Err(Error::Protocol(crate::error::ProtocolError::NetworkOffline));
        }

        // Simulate random failures
        if self.should_fail() {
            return Err(Error::Protocol(crate::error::ProtocolError::Other {
                message: "Network timeout".to_string(),
            }));
        }

        // Simulate latency
        tokio::time::sleep(self.latency).await;

        // Apply rate limiting
        if let Some(limit) = self.rate_limit {
            tokio::time::sleep(limit).await;
        }

        // Return configured response
        self.responses.get(command).cloned().ok_or_else(|| {
            Error::Protocol(crate::error::ProtocolError::Other {
                message: format!("Unknown command: {command}"),
            })
        })
    }

    fn should_fail(&self) -> bool {
        use rand::Rng;
        let mut rng = rand::rngs::ThreadRng::default();
        rng.random::<f32>() < self.failure_rate
    }
}

// ============================================================================
// FixtureGenerator - Generate deterministic test data
// ============================================================================

/// Generator for creating test fixtures with specific properties
pub struct FixtureGenerator {
    temp_dir: Option<PathBuf>,
    seed: u64,
}

impl FixtureGenerator {
    /// Create a new fixture generator
    pub fn new() -> Self {
        Self {
            temp_dir: None,
            seed: 42, // Default seed for determinism
        }
    }
}

impl Default for FixtureGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl FixtureGenerator {
    /// Set seed for deterministic generation
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Generate an anime season (12-13 episodes)
    pub fn generate_anime_season(&mut self, title: &str) -> Vec<TestFileData> {
        let episode_count = 12;
        let mut files = Vec::new();

        for episode in 1..=episode_count {
            files.push(
                TestDataBuilder::anime_episode(title, episode)
                    .with_file_size(350 * 1024 * 1024 + episode as u64 * 1024 * 1024)
                    .build(),
            );
        }

        files
    }

    /// Generate files with specific properties
    pub fn generate_with_properties(&mut self, properties: FileProperties) -> TestFileData {
        TestDataBuilder::new()
            .with_file_size(properties.size)
            .with_content_pattern(properties.pattern)
            .with_anime_title(&properties.title)
            .with_episode_number(properties.episode)
            .build()
    }

    /// Generate file with specific content pattern
    pub fn generate_file_content(&self, size: usize, pattern: ContentPattern) -> Vec<u8> {
        match pattern {
            ContentPattern::Zeros => vec![0u8; size],
            ContentPattern::Random(seed) => {
                use rand::rngs::StdRng;
                use rand::{Rng, SeedableRng};

                let mut rng = StdRng::seed_from_u64(seed);
                let mut data = vec![0u8; size];
                rng.fill(&mut data[..]);
                data
            }
            ContentPattern::Repeating(byte) => vec![byte; size],
            ContentPattern::Gradient => (0..size).map(|i| (i % 256) as u8).collect(),
            ContentPattern::RealVideo => {
                // Simulate video file patterns (header + chunks)
                let mut data = vec![0u8; size];

                // Add mock video header
                if size >= 12 {
                    data[0..4].copy_from_slice(b"ftyp");
                    data[4..8].copy_from_slice(b"isom");
                }

                // Add periodic keyframe patterns
                for i in (1000..size).step_by(10000) {
                    if i + 4 <= size {
                        data[i..i + 4].copy_from_slice(b"mdat");
                    }
                }

                data
            }
        }
    }

    /// Create actual test files in temp directory
    pub fn create_test_files(&mut self, files: &[TestFileData]) -> Result<Vec<PathBuf>, Error> {
        use std::fs;

        // Ensure temp directory exists
        if self.temp_dir.is_none() {
            let temp_path = std::env::temp_dir().join(format!("anidb_test_{}", self.seed));
            fs::create_dir_all(&temp_path).map_err(|e| {
                Error::Io(crate::error::IoError {
                    kind: crate::error::IoErrorKind::Other,
                    path: Some(temp_path.clone()),
                    source: Some(e),
                })
            })?;
            self.temp_dir = Some(temp_path);
        }

        let temp_dir = self.temp_dir.as_ref().unwrap();
        let mut created_paths = Vec::new();

        for file_data in files {
            let file_path = temp_dir.join(file_data.file_path.file_name().unwrap_or_default());

            let content =
                self.generate_file_content(file_data.file_size as usize, file_data.content_pattern);

            fs::write(&file_path, content).map_err(|e| {
                Error::Io(crate::error::IoError {
                    kind: crate::error::IoErrorKind::Other,
                    path: Some(file_path.clone()),
                    source: Some(e),
                })
            })?;

            created_paths.push(file_path);
        }

        Ok(created_paths)
    }

    /// Clean up temporary files
    pub fn cleanup(self) {
        if let Some(temp_dir) = self.temp_dir {
            // Best effort cleanup
            let _ = std::fs::remove_dir_all(temp_dir);
        }
    }
}

/// File properties for generation
#[derive(Debug, Clone)]
pub struct FileProperties {
    pub title: String,
    pub episode: u32,
    pub size: u64,
    pub pattern: ContentPattern,
}

// ============================================================================
// Test execution helpers
// ============================================================================

/// Helper to run scenarios with mock client
pub async fn run_scenario_with_mock(
    scenario: TestScenario,
    _mock_client: MockAniDBClient,
) -> Vec<Result<FileResult, Error>> {
    let mut results = Vec::new();
    let start_time = std::time::Instant::now();

    for (index, file) in scenario.files.iter().enumerate() {
        // Check for planned errors
        let mut has_error = false;
        for error in &scenario.errors {
            if error.at_file_index == Some(index) {
                results.push(Err(convert_error_type(&error.error_type)));
                has_error = true;
                break;
            }

            if let Some(at_time) = error.at_time
                && start_time.elapsed() >= at_time
            {
                results.push(Err(convert_error_type(&error.error_type)));
                has_error = true;
                break;
            }
        }

        if has_error {
            continue;
        }

        // Apply delays
        for delay in &scenario.delays {
            if delay.at_file_index == Some(index) {
                tokio::time::sleep(delay.duration).await;
            }
        }

        // Simulate file processing
        results.push(Ok(FileResult {
            file_path: file.file_path.clone(),
            file_size: file.file_size,
            hashes: file.expected_hashes.clone(),
            status: crate::file_io::ProcessingStatus::Completed,
            processing_time: Duration::from_millis(100),
            anime_info: None,
        }));
    }

    results
}

fn convert_error_type(error_type: &ErrorType) -> Error {
    match error_type {
        ErrorType::FileNotFound => Error::Io(IoError::file_not_found(std::path::Path::new(
            "/test/file.mkv",
        ))),
        ErrorType::PermissionDenied => {
            let io_err =
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Permission denied");
            Error::Io(IoError::permission_denied(
                std::path::Path::new("/test/file.mkv"),
                io_err,
            ))
        }
        ErrorType::NetworkTimeout => Error::Protocol(crate::error::ProtocolError::Other {
            message: "Network timeout".to_string(),
        }),
        ErrorType::CorruptedData => {
            Error::Validation(crate::error::ValidationError::InvalidConfiguration {
                message: "Test corruption".to_string(),
            })
        }
        ErrorType::DiskFull => Error::Io(crate::error::IoError {
            kind: crate::error::IoErrorKind::Other,
            path: Some(std::path::PathBuf::from("/test/file.mkv")),
            source: Some(std::io::Error::other("Disk full")),
        }),
        ErrorType::Custom(msg) => {
            Error::Validation(crate::error::ValidationError::InvalidConfiguration {
                message: msg.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_builder_presets() {
        let small = TestDataBuilder::small_file().build();
        assert!(small.file_size < 1024 * 1024);
        assert!(!small.expected_hashes.is_empty());

        let large = TestDataBuilder::large_file().build();
        assert!(large.file_size > 100 * 1024 * 1024);

        let episode = TestDataBuilder::anime_episode("Test Anime", 5).build();
        assert_eq!(episode.anime_title, "Test Anime");
        assert_eq!(episode.episode_number, 5);
    }

    #[test]
    fn test_scenario_builder_presets() {
        let batch = ScenarioBuilder::batch_processing("One Piece", 5).build();
        assert_eq!(batch.files.len(), 5);

        let perf = ScenarioBuilder::performance_test().build();
        assert_eq!(perf.files.len(), 5);
        assert!(perf.files[0].file_size < perf.files[4].file_size);
    }

    #[test]
    fn test_mock_client_builder() {
        let _client = MockClientBuilder::new()
            .with_standard_responses()
            .with_latency(Duration::from_millis(100))
            .build();

        // Would need async runtime to fully test
    }

    #[test]
    fn test_fixture_generator() {
        let mut generator = FixtureGenerator::new();
        let season = generator.generate_anime_season("Naruto");
        assert_eq!(season.len(), 12);

        let zeros = generator.generate_file_content(100, ContentPattern::Zeros);
        assert_eq!(zeros.len(), 100);
        assert!(zeros.iter().all(|&b| b == 0));
    }
}
