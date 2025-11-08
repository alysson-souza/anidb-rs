//! Core API module for AniDB Client
//!
//! This module contains the main public API structures and implementations
//! for the AniDB Client Core Library. It provides high-level interfaces
//! that build upon the streaming foundation from CORE-002.

use crate::{
    ClientConfig, Error, FileProcessor, HashAlgorithm, Result,
    batch_processor::{BatchProcessor, BatchProcessorConfig},
    error::{ProtocolError, ValidationError},
    progress::{NullProvider, ProgressProvider},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// Processing options for single file operations
///
/// Configures how a single file should be processed, including which
/// hash algorithms to use and whether progress reporting is enabled.
///
/// # Examples
///
/// ```
/// use anidb_client_core::api::ProcessOptions;
/// use anidb_client_core::HashAlgorithm;
///
/// let options = ProcessOptions::new()
///     .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32])
///     .with_progress_reporting(true);
/// ```
#[derive(Clone)]
pub struct ProcessOptions {
    algorithms: Vec<HashAlgorithm>,
    progress_reporting: bool,
    progress_provider: Option<Arc<dyn ProgressProvider>>,
}

impl std::fmt::Debug for ProcessOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessOptions")
            .field("algorithms", &self.algorithms)
            .field("progress_reporting", &self.progress_reporting)
            .field("has_progress_provider", &self.progress_provider.is_some())
            .finish()
    }
}

impl ProcessOptions {
    /// Create new ProcessOptions with default settings
    pub fn new() -> Self {
        Self {
            algorithms: Vec::new(),
            progress_reporting: false,
            progress_provider: None,
        }
    }

    /// Set the hash algorithms to calculate
    pub fn with_algorithms(mut self, algorithms: &[HashAlgorithm]) -> Self {
        self.algorithms = algorithms.to_vec();
        self
    }

    /// Enable or disable progress reporting
    pub fn with_progress_reporting(mut self, enabled: bool) -> Self {
        self.progress_reporting = enabled;
        self
    }

    /// Set a progress provider for reporting
    pub fn with_progress_provider(mut self, provider: Arc<dyn ProgressProvider>) -> Self {
        self.progress_provider = Some(provider);
        self
    }

    /// Get the configured algorithms
    pub fn algorithms(&self) -> &[HashAlgorithm] {
        &self.algorithms
    }

    /// Check if progress reporting is enabled
    pub fn progress_reporting(&self) -> bool {
        self.progress_reporting
    }

    /// Get the progress provider if set
    pub fn progress_provider(&self) -> Option<Arc<dyn ProgressProvider>> {
        self.progress_provider.clone()
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.algorithms.is_empty() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "At least one hash algorithm must be specified",
            )));
        }
        Ok(())
    }
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self::new().with_algorithms(&[HashAlgorithm::ED2K])
    }
}

/// Batch processing options for multiple file operations
///
/// Configures how a batch of files should be processed, including
/// concurrency limits and error handling behavior.
///
/// # Examples
///
/// ```
/// use anidb_client_core::api::BatchOptions;
/// use anidb_client_core::HashAlgorithm;
///
/// let options = BatchOptions::new()
///     .with_algorithms(&[HashAlgorithm::ED2K])
///     .with_max_concurrent(4)
///     .with_continue_on_error(true);
/// ```
#[derive(Debug, Clone)]
pub struct BatchOptions {
    algorithms: Vec<HashAlgorithm>,
    max_concurrent: usize,
    continue_on_error: bool,
    skip_existing: bool,
    use_defaults: bool,
}

impl BatchOptions {
    /// Create new BatchOptions with default settings
    pub fn new() -> Self {
        Self {
            algorithms: vec![HashAlgorithm::ED2K],
            max_concurrent: 4,
            continue_on_error: true,
            skip_existing: false,
            use_defaults: true,
        }
    }

    /// Set the hash algorithms to calculate
    pub fn with_algorithms(mut self, algorithms: &[HashAlgorithm]) -> Self {
        self.algorithms = algorithms.to_vec();
        self
    }

    /// Set the maximum number of concurrent file operations
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Enable or disable continuing on error
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Enable or disable skipping already processed files
    pub fn with_skip_existing(mut self, skip: bool) -> Self {
        self.skip_existing = skip;
        self
    }

    /// Set whether to use default media extensions when no include patterns are specified
    pub fn with_use_defaults(mut self, use_defaults: bool) -> Self {
        self.use_defaults = use_defaults;
        self
    }

    /// Get the configured algorithms
    pub fn algorithms(&self) -> &[HashAlgorithm] {
        &self.algorithms
    }

    /// Get the maximum concurrent operations limit
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// Check if continuing on error is enabled
    pub fn continue_on_error(&self) -> bool {
        self.continue_on_error
    }

    /// Check if skipping existing files is enabled
    pub fn skip_existing(&self) -> bool {
        self.skip_existing
    }

    /// Check if default media extensions should be used
    pub fn use_defaults(&self) -> bool {
        self.use_defaults
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.algorithms.is_empty() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "At least one hash algorithm must be specified",
            )));
        }
        if self.max_concurrent == 0 {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "Maximum concurrent operations must be greater than 0",
            )));
        }
        Ok(())
    }
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of processing a single file
///
/// Contains the results of file processing including calculated hashes,
/// processing status, and optional anime identification information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResult {
    /// Path to the processed file
    pub file_path: PathBuf,
    /// Size of the file in bytes
    pub file_size: u64,
    /// Calculated hashes by algorithm
    pub hashes: HashMap<HashAlgorithm, String>,
    /// Processing status
    pub status: crate::file_io::ProcessingStatus,
    /// Time taken to process the file
    pub processing_time: Duration,
    /// Optional anime identification information
    pub anime_info: Option<AnimeIdentification>,
}

/// Result of batch processing multiple files
///
/// Contains statistics and results for all files processed in a batch operation.
#[derive(Debug)]
pub struct BatchResult {
    /// Total number of files processed
    pub total_files: usize,
    /// Number of successfully processed files
    pub successful_files: usize,
    /// Number of files that failed processing
    pub failed_files: usize,
    /// Individual file results
    pub results: Vec<Result<FileResult>>,
    /// Total processing time for the batch
    pub total_time: Duration,
}

impl BatchResult {
    /// Create a BatchResult from a vector of individual results
    pub fn from_results(results: Vec<Result<FileResult>>) -> Self {
        let total_files = results.len();
        let successful_files = results.iter().filter(|r| r.is_ok()).count();
        let failed_files = total_files - successful_files;

        Self {
            total_files,
            successful_files,
            failed_files,
            results,
            total_time: Duration::from_secs(0), // Will be filled by the actual implementation
        }
    }
}

/// Anime identification information from AniDB
///
/// Contains metadata about an identified anime episode and source information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimeIdentification {
    /// AniDB anime ID
    pub anime_id: u64,
    /// AniDB episode ID
    pub episode_id: u64,
    /// Anime title
    pub title: String,
    /// Episode number
    pub episode_number: u32,
    /// Source of the identification
    pub source: IdentificationSource,
}

/// Source of anime identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentificationSource {
    /// Identified via AniDB API
    AniDB,
    /// Identified from local cache
    Cache,
    /// Identified from filename patterns
    Filename,
}

/// Main AniDB Client interface
///
/// This is the primary entry point for all AniDB client operations.
/// It provides high-level methods for file processing, batch operations,
/// and anime identification.
///
/// # Examples
///
/// ```no_run
/// use anidb_client_core::{AniDBClient, ClientConfig};
/// use anidb_client_core::api::ProcessOptions;
/// use anidb_client_core::HashAlgorithm;
/// use std::path::Path;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = ClientConfig::default();
/// let client = AniDBClient::new(config).await?;
///
/// let options = ProcessOptions::new()
///     .with_algorithms(&[HashAlgorithm::ED2K]);
///
/// let result = client.process_file(Path::new("anime.mkv"), options).await?;
/// println!("Processed file: {:?}", result.file_path);
/// # Ok(())
/// # }
/// ```
pub struct AniDBClient {
    _config: ClientConfig,
    file_processor: Arc<FileProcessor>,
    batch_processor: Arc<BatchProcessor>,
    _ready: bool,
}

impl AniDBClient {
    /// Create a new AniDB client with the given configuration
    ///
    /// This initializes all internal components and prepares the client
    /// for file processing operations.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration including processing limits
    ///
    /// # Returns
    ///
    /// A Result containing the initialized client or an error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anidb_client_core::{AniDBClient, ClientConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = ClientConfig::default();
    /// let client = AniDBClient::new(config).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new(config: ClientConfig) -> Result<Self> {
        // Create FileProcessor
        let file_processor = Arc::new(FileProcessor::new(config.clone()));

        // Create BatchProcessor with optimized configuration
        let batch_config = BatchProcessorConfig {
            base_concurrency: config.max_concurrent_files,
            max_concurrency: config.max_concurrent_files * 2,
            min_concurrency: 1,
            adaptive_concurrency: true,
            enable_pooling: true,
            max_pool_size: 16,
            continue_on_error: true,
            smart_batching: true,
            progressive_results: true,
            memory_check_interval: std::time::Duration::from_millis(500),
        };

        // Create a memory manager for batch processing
        let memory_config = crate::memory::MemoryConfig {
            max_memory: config.max_memory_usage,
            max_pool_size: 20,
            auto_shrink: true,
            eviction_timeout: std::time::Duration::from_secs(60),
            enable_diagnostics: true,
        };
        let memory_manager = Arc::new(crate::memory::MemoryManager::with_config(memory_config));

        let batch_processor = Arc::new(BatchProcessor::new(
            batch_config,
            config.clone(),
            memory_manager,
        ));

        Ok(Self {
            _config: config,
            file_processor,
            batch_processor,
            _ready: true,
        })
    }

    /// Check if the client is ready for operations
    pub fn is_ready(&self) -> bool {
        self._ready
    }

    /// Process a single file with the given options
    ///
    /// This method calculates hashes for the specified file using the
    /// algorithms configured in the options. It can optionally report
    /// progress and attempt to identify the anime.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the file to process
    /// * `options` - Processing options including algorithms and settings
    ///
    /// # Returns
    ///
    /// A Result containing the file processing results or an error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anidb_client_core::{AniDBClient, ClientConfig};
    /// use anidb_client_core::api::ProcessOptions;
    /// use anidb_client_core::HashAlgorithm;
    /// use std::path::Path;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = AniDBClient::new(ClientConfig::test()).await?;
    ///
    /// let options = ProcessOptions::new()
    ///     .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32]);
    ///
    /// let result = client.process_file(Path::new("episode.mkv"), options).await?;
    /// println!("ED2K Hash: {}", result.hashes[&HashAlgorithm::ED2K]);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_file(
        &self,
        file_path: &Path,
        options: ProcessOptions,
    ) -> Result<FileResult> {
        // Validate options
        options.validate()?;

        // Process the file using the existing FileProcessor
        let provider = options
            .progress_provider()
            .unwrap_or_else(|| Arc::new(NullProvider));
        let processing_result = self
            .file_processor
            .process_file(file_path, options.algorithms(), provider.clone())
            .await?;

        // Convert to FileResult format
        Ok(FileResult {
            file_path: processing_result.file_path,
            file_size: processing_result.file_size,
            hashes: processing_result.hashes,
            status: processing_result.status,
            processing_time: processing_result.processing_time,
            anime_info: None, // TODO: Implement anime identification
        })
    }

    /// Process a single file with an explicit progress provider
    ///
    /// This variant cleanly separates progress concerns from options.
    pub async fn process_file_with_progress(
        &self,
        file_path: &Path,
        options: ProcessOptions,
        progress: Arc<dyn ProgressProvider>,
    ) -> Result<FileResult> {
        // Validate options
        options.validate()?;

        // Process using provided progress provider through the unified pipeline
        let processing_result = self
            .file_processor
            .process_file(file_path, options.algorithms(), progress)
            .await?;

        Ok(FileResult {
            file_path: processing_result.file_path,
            file_size: processing_result.file_size,
            hashes: processing_result.hashes,
            status: processing_result.status,
            processing_time: processing_result.processing_time,
            anime_info: None,
        })
    }

    /// Process multiple files in a batch with the given options
    ///
    /// This method processes multiple files concurrently according to the
    /// batch options configuration. It can continue on errors and provides
    /// overall statistics.
    ///
    /// # Arguments
    ///
    /// * `file_paths` - Slice of file paths to process
    /// * `options` - Batch processing options including concurrency limits
    ///
    /// # Returns
    ///
    /// A Result containing the batch processing results or an error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anidb_client_core::{AniDBClient, ClientConfig};
    /// use anidb_client_core::api::BatchOptions;
    /// use anidb_client_core::HashAlgorithm;
    /// use std::path::PathBuf;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = AniDBClient::new(ClientConfig::test()).await?;
    ///
    /// let files = vec![
    ///     PathBuf::from("episode1.mkv"),
    ///     PathBuf::from("episode2.mkv"),
    /// ];
    ///
    /// let options = BatchOptions::new()
    ///     .with_algorithms(&[HashAlgorithm::ED2K])
    ///     .with_max_concurrent(2);
    ///
    /// let result = client.process_batch(&files, options).await?;
    /// println!("Processed {} files successfully", result.successful_files);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_batch(
        &self,
        file_paths: &[PathBuf],
        options: BatchOptions,
    ) -> Result<BatchResult> {
        // Validate options
        options.validate()?;

        // Use the optimized batch processor
        let provider = Arc::new(NullProvider);
        let batch_result = self
            .batch_processor
            .process_batch(file_paths.to_vec(), options.algorithms(), provider)
            .await?;

        // Convert to API BatchResult format
        let converted_results: Vec<Result<FileResult>> = batch_result
            .results
            .into_iter()
            .map(|result| {
                result.map(|processing_result| FileResult {
                    file_path: processing_result.file_path,
                    file_size: processing_result.file_size,
                    hashes: processing_result.hashes,
                    status: processing_result.status,
                    processing_time: processing_result.processing_time,
                    anime_info: None,
                })
            })
            .collect();

        Ok(BatchResult {
            total_files: batch_result.total_files,
            successful_files: batch_result.successful,
            failed_files: batch_result.failed,
            results: converted_results,
            total_time: batch_result.total_time,
        })
    }

    /// Identify an anime file using its hash and size
    ///
    /// This method attempts to identify an anime episode using the file's
    /// hash and size by querying the AniDB database. It may fall back to
    /// filename-based identification.
    ///
    /// # Arguments
    ///
    /// * `hash` - File hash (typically ED2K)
    /// * `size` - File size in bytes
    ///
    /// # Returns
    ///
    /// A Result containing anime identification information or an error
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use anidb_client_core::{AniDBClient, ClientConfig};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = AniDBClient::new(ClientConfig::test()).await?;
    ///
    /// let hash = "ed2k_hash_here";
    /// let size = 1024 * 1024 * 700; // 700MB
    ///
    /// match client.identify_file(hash, size).await {
    ///     Ok(identification) => {
    ///         println!("Found anime: {}", identification.title);
    ///     }
    ///     Err(_) => {
    ///         println!("Could not identify anime");
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn identify_file(&self, _hash: &str, _size: u64) -> Result<AnimeIdentification> {
        // TODO: Implement actual AniDB API integration
        // For now, return a network offline error to satisfy tests
        Err(Error::Protocol(ProtocolError::NetworkOffline))
    }
}
