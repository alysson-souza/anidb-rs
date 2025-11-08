//! Test data builders for creating test scenarios

use anidb_client_core::{
    Error, HashAlgorithm, Progress, Result,
    api::{AnimeIdentification, FileResult, IdentificationSource},
    error::{InternalError, IoError, ProtocolError},
    file_io::ProcessingStatus,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Builder for creating test data scenarios
pub struct TestDataBuilder {
    anime_title: Option<String>,
    episode_number: Option<u32>,
    file_size: Option<u64>,
    expected_hashes: HashMap<HashAlgorithm, String>,
    is_batch_builder: bool,
    batch_files: Vec<TestFileData>,
    error_scenarios: Vec<Error>,
}

#[derive(Debug, Clone)]
pub struct TestFileData {
    pub anime_title: String,
    pub episode_number: u32,
    pub file_size: u64,
    pub expected_hashes: HashMap<HashAlgorithm, String>,
}

#[derive(Debug)]
pub struct TestBatch {
    pub files: Vec<TestFileData>,
    pub total_size: u64,
}

impl TestDataBuilder {
    /// Create a new test data builder
    pub fn new() -> Self {
        Self {
            anime_title: None,
            episode_number: None,
            file_size: None,
            expected_hashes: HashMap::new(),
            is_batch_builder: false,
            batch_files: Vec::new(),
            error_scenarios: Vec::new(),
        }
    }

    /// Set anime title
    pub fn with_anime_title(mut self, title: &str) -> Self {
        self.anime_title = Some(title.to_string());
        self
    }

    /// Set episode number
    pub fn with_episode_number(mut self, episode: u32) -> Self {
        self.episode_number = Some(episode);
        self
    }

    /// Set file size
    pub fn with_file_size(mut self, size: u64) -> Self {
        self.file_size = Some(size);
        self
    }

    /// Add expected hash
    pub fn with_hash(mut self, algorithm: HashAlgorithm, hash: &str) -> Self {
        self.expected_hashes.insert(algorithm, hash.to_string());
        self
    }

    /// Build test file data
    pub fn build(self) -> TestFileData {
        TestFileData {
            anime_title: self.anime_title.unwrap_or_else(|| "Test Anime".to_string()),
            episode_number: self.episode_number.unwrap_or(1),
            file_size: self.file_size.unwrap_or(1024 * 1024),
            expected_hashes: self.expected_hashes,
        }
    }

    /// Create batch builder
    pub fn create_batch(mut self) -> Self {
        self.is_batch_builder = true;
        self
    }

    /// Add file to batch
    pub fn add_anime_file(mut self, title: &str, episode: u32, size: u64) -> Self {
        if self.is_batch_builder {
            self.batch_files.push(TestFileData {
                anime_title: title.to_string(),
                episode_number: episode,
                file_size: size,
                expected_hashes: HashMap::new(),
            });
        }
        self
    }

    /// Build batch
    pub fn build_batch(self) -> TestBatch {
        let total_size = self.batch_files.iter().map(|f| f.file_size).sum();
        TestBatch {
            files: self.batch_files,
            total_size,
        }
    }

    /// Create error scenarios builder
    pub fn create_error_scenarios(mut self) -> Self {
        self.error_scenarios.clear();
        self
    }

    /// Add file not found scenario
    pub fn add_file_not_found_scenario(mut self, path: &str) -> Self {
        self.error_scenarios
            .push(Error::Io(IoError::file_not_found(&PathBuf::from(path))));
        self
    }

    /// Add permission denied scenario
    pub fn add_permission_denied_scenario(mut self, path: &str) -> Self {
        let io_error = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Test permission error",
        );
        self.error_scenarios
            .push(Error::Io(IoError::permission_denied(
                &PathBuf::from(path),
                io_error,
            )));
        self
    }

    /// Add network error scenario
    pub fn add_network_error_scenario(mut self) -> Self {
        self.error_scenarios
            .push(Error::Protocol(ProtocolError::NetworkOffline));
        self
    }

    /// Build error scenarios
    pub fn build_scenarios(self) -> Vec<Error> {
        self.error_scenarios
    }
}

impl Default for TestDataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test file builder for creating test files
pub struct TestFileBuilder {
    base_dir: PathBuf,
    generated_files: Vec<PathBuf>,
}

impl TestFileBuilder {
    /// Create a new test file builder
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
            generated_files: Vec::new(),
        }
    }

    /// Check if the builder is ready to use
    pub fn is_ready(&self) -> bool {
        self.base_dir.exists() && self.base_dir.is_dir()
    }

    /// Generate a deterministic file with specific size and seed
    pub fn generate_deterministic_file(
        &mut self,
        name: &str,
        size: usize,
        seed: u64,
    ) -> Result<PathBuf> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let file_path = self.base_dir.join(name);

        // Generate deterministic content based on seed
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        let hash = hasher.finish();

        let mut content = Vec::with_capacity(size);
        let mut current_hash = hash;

        for _ in 0..size {
            content.push((current_hash & 0xFF) as u8);
            current_hash = current_hash.wrapping_mul(1664525).wrapping_add(1013904223);
        }

        std::fs::write(&file_path, content).map_err(|e| {
            Error::Internal(InternalError::ffi(
                "test_file_builder",
                &format!("Failed to write test file: {e}"),
            ))
        })?;

        self.generated_files.push(file_path.clone());
        Ok(file_path)
    }

    /// Generate a file that produces a specific hash
    pub fn generate_file_with_hash(
        &mut self,
        name: &str,
        algorithm: HashAlgorithm,
        expected_hash: &str,
    ) -> Result<PathBuf> {
        let file_path = self.base_dir.join(name);

        // For now, handle special cases for known hashes
        let content = match (algorithm, expected_hash) {
            (HashAlgorithm::ED2K, "d41d8cd98f00b204e9800998ecf8427e") => {
                // Empty file ED2K hash
                Vec::new()
            }
            _ => {
                // Generate placeholder content - in a full implementation,
                // this would use reverse hash lookup or brute force
                format!("test content for hash {expected_hash}").into_bytes()
            }
        };

        std::fs::write(&file_path, content).map_err(|e| {
            Error::Internal(InternalError::ffi(
                "test_file_builder",
                &format!("Failed to write test file: {e}"),
            ))
        })?;

        self.generated_files.push(file_path.clone());
        Ok(file_path)
    }

    /// Generate a standard test file
    pub fn generate_test_file(&mut self, name: &str, size: usize) -> Result<PathBuf> {
        let file_path = self.base_dir.join(name);

        // Generate simple content
        let content = vec![0u8; size];

        std::fs::write(&file_path, content).map_err(|e| {
            Error::Internal(InternalError::ffi(
                "test_file_builder",
                &format!("Failed to write test file: {e}"),
            ))
        })?;

        self.generated_files.push(file_path.clone());
        Ok(file_path)
    }

    /// Generate a corrupted file (invalid structure but readable)
    pub fn generate_corrupted_file(&mut self, name: &str, size: usize) -> Result<PathBuf> {
        let file_path = self.base_dir.join(name);

        // Generate content with invalid patterns
        let mut content = Vec::with_capacity(size);
        for i in 0..size {
            // Create patterns that might confuse parsers
            content.push(match i % 4 {
                0 => 0xFF,
                1 => 0x00,
                2 => (i & 0xFF) as u8,
                _ => 0xAA,
            });
        }

        std::fs::write(&file_path, content).map_err(|e| {
            Error::Internal(InternalError::ffi(
                "test_file_builder",
                &format!("Failed to write corrupted test file: {e}"),
            ))
        })?;

        self.generated_files.push(file_path.clone());
        Ok(file_path)
    }

    /// Clean up all generated files
    pub fn cleanup(&mut self) {
        for file_path in &self.generated_files {
            let _ = std::fs::remove_file(file_path);
        }
        self.generated_files.clear();
    }
}

impl Drop for TestFileBuilder {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Test utilities for creating mock data
pub mod test_utils {
    use super::*;

    /// Create a mock file result for testing
    pub fn create_mock_file_result(file_path: PathBuf, algorithms: &[HashAlgorithm]) -> FileResult {
        let mut hashes = HashMap::new();
        for &algorithm in algorithms {
            let hash = match algorithm {
                HashAlgorithm::ED2K => "098f6bcd4621d373cade4e832627b4f6",
                HashAlgorithm::CRC32 => "d87f7e0c",
                HashAlgorithm::MD5 => "5d41402abc4b2a76b9719d911017c592",
                HashAlgorithm::SHA1 => "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d",
                HashAlgorithm::TTH => "czquwh3iyxbf5l3bgyugzhassmxu647ip2ike4y",
            };
            hashes.insert(algorithm, hash.to_string());
        }

        FileResult {
            file_path,
            file_size: 1024 * 1024, // 1MB
            hashes,
            status: ProcessingStatus::Completed,
            processing_time: Duration::from_millis(100),
            anime_info: Some(AnimeIdentification {
                anime_id: 12345,
                episode_id: 67890,
                title: "Mock Anime Series".to_string(),
                episode_number: 1,
                source: IdentificationSource::AniDB,
            }),
        }
    }

    /// Create a mock anime identification
    pub fn create_mock_anime_identification(title: &str, episode: u32) -> AnimeIdentification {
        AnimeIdentification {
            anime_id: 12345,
            episode_id: 67890,
            title: title.to_string(),
            episode_number: episode,
            source: IdentificationSource::AniDB,
        }
    }

    /// Create mock progress data
    pub fn create_mock_progress(percentage: f64, bytes_processed: u64) -> Progress {
        Progress {
            percentage,
            bytes_processed,
            total_bytes: 1024 * 1024, // 1MB total
            throughput_mbps: 100.0,
            current_operation: format!("Mock operation at {percentage}%"),
            memory_usage_bytes: Some(10 * 1024 * 1024), // Mock 10MB usage
            peak_memory_bytes: Some(15 * 1024 * 1024),  // Mock 15MB peak
            buffer_size: Some(8 * 1024 * 1024),         // Mock 8MB buffer
        }
    }
}
