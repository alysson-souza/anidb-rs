//! Optimized batch file processing with resource pooling and adaptive concurrency
//!
//! This module provides efficient batch processing of files with:
//! - Resource pooling to avoid pipeline recreation overhead
//! - Adaptive concurrency based on memory usage and file sizes
//! - Progressive result streaming
//! - Smart batching by file size
//! - Error resilience

use crate::{
    ClientConfig, Error, HashAlgorithm, Result,
    error::{InternalError, IoError},
    file_io::{FileProcessingResult, ProcessingStatus},
    memory::{MEMORY_WARNING_THRESHOLD, MemoryManager},
    pipeline::{
        HashingStage, PipelineConfig, StreamingPipeline, StreamingPipelineBuilder, ValidationStage,
    },
    progress::ProgressProvider,
};
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

/// Result of batch processing with progressive updates
#[derive(Debug)]
pub struct BatchProcessingResult {
    /// Total files to process
    pub total_files: usize,
    /// Successfully processed files
    pub successful: usize,
    /// Failed files
    pub failed: usize,
    /// Total processing time
    pub total_time: Duration,
    /// Individual results
    pub results: Vec<Result<FileProcessingResult>>,
}

/// Configuration for batch processing optimization
#[derive(Debug, Clone)]
pub struct BatchProcessorConfig {
    /// Base concurrency level
    pub base_concurrency: usize,
    /// Maximum concurrency level
    pub max_concurrency: usize,
    /// Minimum concurrency level
    pub min_concurrency: usize,
    /// Enable adaptive concurrency
    pub adaptive_concurrency: bool,
    /// Enable resource pooling
    pub enable_pooling: bool,
    /// Maximum pool size per algorithm set
    pub max_pool_size: usize,
    /// Continue processing on error
    pub continue_on_error: bool,
    /// Group files by size for better resource allocation
    pub smart_batching: bool,
    /// Progressive result reporting
    pub progressive_results: bool,
    /// Memory check interval
    pub memory_check_interval: Duration,
}

impl Default for BatchProcessorConfig {
    fn default() -> Self {
        Self {
            base_concurrency: 4,
            max_concurrency: 8,
            min_concurrency: 1,
            adaptive_concurrency: true,
            enable_pooling: true,
            max_pool_size: 16,
            continue_on_error: true,
            smart_batching: true,
            progressive_results: true,
            memory_check_interval: Duration::from_millis(500),
        }
    }
}

/// File size categories for smart batching
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FileSizeCategory {
    Small,  // < 100MB
    Medium, // 100MB - 1GB
    Large,  // 1GB - 10GB
    Huge,   // > 10GB
}

impl FileSizeCategory {
    fn from_size(size: u64) -> Self {
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * MB;

        match size {
            s if s < 100 * MB => FileSizeCategory::Small,
            s if s < GB => FileSizeCategory::Medium,
            s if s < 10 * GB => FileSizeCategory::Large,
            _ => FileSizeCategory::Huge,
        }
    }

    /// Get recommended concurrency for this file size
    fn recommended_concurrency(&self) -> usize {
        match self {
            FileSizeCategory::Small => 8,
            FileSizeCategory::Medium => 4,
            FileSizeCategory::Large => 2,
            FileSizeCategory::Huge => 1,
        }
    }
}

/// Information about a file to process
#[derive(Debug, Clone)]
struct FileInfo {
    path: PathBuf,
    size: u64,
    category: FileSizeCategory,
}

impl FileInfo {
    async fn from_path(path: PathBuf) -> Result<Self> {
        let metadata = tokio::fs::metadata(&path).await.map_err(Error::from)?;

        let size = metadata.len();
        let category = FileSizeCategory::from_size(size);

        Ok(Self {
            path,
            size,
            category,
        })
    }
}

/// Pool for reusing StreamingPipeline instances
struct PipelinePool {
    /// Available pipelines
    available: Arc<Mutex<VecDeque<StreamingPipeline>>>,
    /// Configuration for creating new pipelines
    config: ClientConfig,
    /// Algorithms for this pool
    algorithms: Vec<HashAlgorithm>,
    /// Maximum pool size
    max_size: usize,
    /// Current pool size
    current_size: Arc<Mutex<usize>>,
}

impl PipelinePool {
    fn new(config: ClientConfig, algorithms: Vec<HashAlgorithm>, max_size: usize) -> Self {
        Self {
            available: Arc::new(Mutex::new(VecDeque::new())),
            config,
            algorithms,
            max_size,
            current_size: Arc::new(Mutex::new(0)),
        }
    }

    /// Acquire a pipeline from the pool or create a new one
    async fn acquire(&self) -> Result<StreamingPipeline> {
        // Try to get from pool first
        if let Some(pipeline) = self.available.lock().await.pop_front() {
            return Ok(pipeline);
        }

        // Create new pipeline if under limit
        let mut size = self.current_size.lock().await;
        if *size < self.max_size {
            *size += 1;
            drop(size);

            // Create new pipeline with configured algorithms
            let preferred_chunk = if self.algorithms.contains(&HashAlgorithm::ED2K) {
                9_728_000 // ED2K chunk size
            } else if self.algorithms.len() > 1 {
                1024 * 1024 // 1MB for multiple algorithms
            } else {
                self.config.chunk_size
            };

            let pipeline_config = PipelineConfig {
                chunk_size: preferred_chunk,
                parallel_stages: false,
                max_memory: self.config.max_memory_usage,
            };

            // Create validation stage
            let validation_stage = Box::new(
                ValidationStage::new()
                    .with_max_file_size(100 * 1024 * 1024 * 1024) // 100GB max
                    .reject_empty_chunks(false),
            );

            // Create hashing stage with all algorithms
            let hashing_stage = Box::new(HashingStage::new(&self.algorithms));

            // Build pipeline with stages
            let pipeline = StreamingPipelineBuilder::with_config(pipeline_config)
                .add_stage(validation_stage)
                .add_stage(hashing_stage)
                .build();

            Ok(pipeline)
        } else {
            // Pool is at capacity, create a new one anyway to avoid deadlock
            // This can happen when all pipelines are in use
            // Create new pipeline with configured algorithms
            let preferred_chunk = if self.algorithms.contains(&HashAlgorithm::ED2K) {
                9_728_000
            } else if self.algorithms.len() > 1 {
                1024 * 1024
            } else {
                self.config.chunk_size
            };

            let pipeline_config = PipelineConfig {
                chunk_size: preferred_chunk,
                parallel_stages: false,
                max_memory: self.config.max_memory_usage,
            };

            // Create validation stage
            let validation_stage = Box::new(
                ValidationStage::new()
                    .with_max_file_size(100 * 1024 * 1024 * 1024) // 100GB max
                    .reject_empty_chunks(false),
            );

            // Create hashing stage with all algorithms
            let hashing_stage = Box::new(HashingStage::new(&self.algorithms));

            // Build pipeline with stages
            let pipeline = StreamingPipelineBuilder::with_config(pipeline_config)
                .add_stage(validation_stage)
                .add_stage(hashing_stage)
                .build();

            Ok(pipeline)
        }
    }

    /// Release a pipeline back to the pool
    async fn release(&self, pipeline: StreamingPipeline) {
        // Return to pool if there's space
        let mut available = self.available.lock().await;
        if available.len() < self.max_size {
            available.push_back(pipeline);
        } else {
            // Pool is full, decrease size counter
            let mut size = self.current_size.lock().await;
            *size = (*size).saturating_sub(1);
        }
    }
}

/// Adaptive concurrency controller
struct ConcurrencyController {
    memory_manager: Arc<MemoryManager>,
    #[allow(dead_code)]
    base_concurrency: usize,
    max_concurrency: usize,
    min_concurrency: usize,
    current_concurrency: Arc<Mutex<usize>>,
    last_check: Arc<Mutex<Instant>>,
    check_interval: Duration,
}

impl ConcurrencyController {
    fn new(
        memory_manager: Arc<MemoryManager>,
        base_concurrency: usize,
        max_concurrency: usize,
        min_concurrency: usize,
        check_interval: Duration,
    ) -> Self {
        Self {
            memory_manager,
            base_concurrency,
            max_concurrency,
            min_concurrency,
            current_concurrency: Arc::new(Mutex::new(base_concurrency)),
            last_check: Arc::new(Mutex::new(Instant::now())),
            check_interval,
        }
    }

    /// Get current recommended concurrency level
    async fn get_concurrency(&self, file_category: FileSizeCategory) -> usize {
        // Check if we should update concurrency
        let mut last_check = self.last_check.lock().await;
        if last_check.elapsed() > self.check_interval {
            *last_check = Instant::now();
            drop(last_check);

            // Update based on memory usage
            let usage_percent = self.memory_manager.memory_usage_percent() / 100.0;
            let mut current = self.current_concurrency.lock().await;

            if usage_percent > MEMORY_WARNING_THRESHOLD {
                // Reduce concurrency when memory pressure is high
                *current = (*current).saturating_sub(1).max(self.min_concurrency);
            } else if usage_percent < 0.5 {
                // Increase concurrency when memory usage is low
                *current = (*current + 1).min(self.max_concurrency);
            }
        }

        // Adjust based on file size category
        let base = *self.current_concurrency.lock().await;
        let recommended = file_category.recommended_concurrency();
        base.min(recommended)
    }
}

/// Optimized batch processor for multiple files
pub struct BatchProcessor {
    config: BatchProcessorConfig,
    client_config: ClientConfig,
    #[allow(dead_code)]
    memory_manager: Arc<MemoryManager>,
    pipeline_pools: Arc<Mutex<std::collections::HashMap<Vec<HashAlgorithm>, Arc<PipelinePool>>>>,
    concurrency_controller: Option<Arc<ConcurrencyController>>,
}

impl BatchProcessor {
    /// Create a new batch processor
    pub fn new(
        config: BatchProcessorConfig,
        client_config: ClientConfig,
        memory_manager: Arc<MemoryManager>,
    ) -> Self {
        let concurrency_controller = if config.adaptive_concurrency {
            Some(Arc::new(ConcurrencyController::new(
                memory_manager.clone(),
                config.base_concurrency,
                config.max_concurrency,
                config.min_concurrency,
                config.memory_check_interval,
            )))
        } else {
            None
        };

        Self {
            config,
            client_config,
            memory_manager,
            pipeline_pools: Arc::new(Mutex::new(std::collections::HashMap::new())),
            concurrency_controller,
        }
    }

    /// Process a batch of files with optimizations
    pub async fn process_batch(
        &self,
        file_paths: Vec<PathBuf>,
        algorithms: &[HashAlgorithm],
        progress_provider: Arc<dyn ProgressProvider>,
    ) -> Result<BatchProcessingResult> {
        let start_time = Instant::now();
        let total_files = file_paths.len();
        let original_paths = file_paths.clone();

        // Get file information and sort by size if smart batching is enabled
        let mut file_infos = self.collect_file_info(file_paths).await?;
        if self.config.smart_batching {
            file_infos.sort_by_key(|f| (f.category, f.size));
        }

        // Get or create pipeline pool for these algorithms
        let pool = self.get_or_create_pool(algorithms.to_vec()).await;

        // Set up concurrency control
        let concurrency = self.get_initial_concurrency(&file_infos).await;
        let semaphore = Arc::new(Semaphore::new(concurrency));

        // Prepare result aggregation
        let mut results: Vec<Result<FileProcessingResult>> = Vec::with_capacity(total_files);
        let mut successful = 0usize;
        let mut failed = 0usize;

        // Pre-account for any files that were skipped during info collection (e.g., nonexistent)
        // We treat those as immediate failures to keep counts consistent with input size
        let skipped = total_files.saturating_sub(file_infos.len());
        if skipped > 0 {
            // Identify missing paths for better error detail (best-effort; cheap check)
            let mut remaining_skip = skipped;
            for p in &original_paths {
                if remaining_skip == 0 {
                    break;
                }
                if !p.exists() {
                    results.push(Err(Error::Io(IoError::file_not_found(p))));
                    failed += 1;
                    remaining_skip -= 1;
                    if self.config.progressive_results {
                        let _progress = (results.len() as f64 / total_files as f64) * 100.0;
                        progress_provider.report(crate::progress::ProgressUpdate::FileProgress {
                            path: std::path::PathBuf::from("batch"),
                            bytes_processed: results.len() as u64,
                            total_bytes: total_files as u64,
                            operation: format!("Processed {}/{} files", results.len(), total_files),
                            throughput_mbps: None,
                            memory_usage_bytes: None,
                            buffer_size: None,
                        });
                    }
                }
            }
            // Fallback for any remaining skips (unknown reasons)
            for _ in 0..remaining_skip {
                results.push(Err(Error::Internal(InternalError::assertion(
                    "File skipped during info collection",
                ))));
                failed += 1;
                if self.config.progressive_results {
                    let _progress = (results.len() as f64 / total_files as f64) * 100.0;
                    progress_provider.report(crate::progress::ProgressUpdate::FileProgress {
                        path: std::path::PathBuf::from("batch"),
                        bytes_processed: results.len() as u64,
                        total_bytes: total_files as u64,
                        operation: format!("Processed {}/{} files", results.len(), total_files),
                        throughput_mbps: None,
                        memory_usage_bytes: None,
                        buffer_size: None,
                    });
                }
            }
        }

        // Process files concurrently
        let mut futures = FuturesUnordered::new();

        for file_info in file_infos {
            let pool = pool.clone();
            let algorithms = algorithms.to_vec();
            let progress =
                progress_provider.create_child(&format!("File: {}", file_info.path.display()));
            let continue_on_error = self.config.continue_on_error;
            let semaphore_cloned = semaphore.clone();

            futures.push(async move {
                // Acquire permit inside the task to avoid blocking the producer loop
                let permit = semaphore_cloned.acquire_owned().await.map_err(|_e| {
                    Error::Internal(InternalError::assertion("Failed to acquire semaphore"))
                })?;

                let result = Self::process_single_file(
                    pool,
                    file_info.clone(),
                    algorithms,
                    Arc::from(progress),
                )
                .await;

                drop(permit);
                // Propagate hard errors only if continue_on_error is false
                match result {
                    Ok(r) => Ok(Ok(r)),
                    Err(e) => {
                        if continue_on_error {
                            Ok(Err(e))
                        } else {
                            Err(e)
                        }
                    }
                }
            });
        }

        // Drive futures and collect results in a single loop to avoid deadlock
        while let Some(next) = futures.next().await {
            match next {
                // Future completed and returned either a successful or failed file result
                Ok(inner) => match inner {
                    Ok(file_result) => {
                        // Classify based on ProcessingStatus
                        if file_result.status == ProcessingStatus::Completed {
                            successful += 1;
                        } else {
                            failed += 1;
                        }
                        results.push(Ok(file_result));
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(Err(e));
                    }
                },
                // Hard error from the task construction (only when continue_on_error == false)
                Err(e) => {
                    return Err(e);
                }
            }

            // Report progress if enabled
            if self.config.progressive_results {
                let _progress = (results.len() as f64 / total_files as f64) * 100.0;
                progress_provider.report(crate::progress::ProgressUpdate::FileProgress {
                    path: std::path::PathBuf::from("batch"),
                    bytes_processed: results.len() as u64,
                    total_bytes: total_files as u64,
                    operation: format!("Processed {}/{} files", results.len(), total_files),
                    throughput_mbps: None,
                    memory_usage_bytes: None,
                    buffer_size: None,
                });
            }
        }

        Ok(BatchProcessingResult {
            total_files,
            successful,
            failed,
            total_time: start_time.elapsed(),
            results,
        })
    }

    /// Collect file information for all paths
    async fn collect_file_info(&self, paths: Vec<PathBuf>) -> Result<Vec<FileInfo>> {
        let mut infos = Vec::with_capacity(paths.len());
        for path in paths {
            match FileInfo::from_path(path).await {
                Ok(info) => infos.push(info),
                Err(e) if self.config.continue_on_error => {
                    // Log error but continue
                    eprintln!("Failed to get file info: {e}");
                }
                Err(e) => return Err(e),
            }
        }
        Ok(infos)
    }

    /// Get or create a pipeline pool for the given algorithms
    async fn get_or_create_pool(&self, algorithms: Vec<HashAlgorithm>) -> Arc<PipelinePool> {
        let mut pools = self.pipeline_pools.lock().await;

        if let Some(pool) = pools.get(&algorithms) {
            pool.clone()
        } else {
            let pool = Arc::new(PipelinePool::new(
                self.client_config.clone(),
                algorithms.clone(),
                self.config.max_pool_size,
            ));
            pools.insert(algorithms, pool.clone());
            pool
        }
    }

    /// Get initial concurrency based on file sizes
    async fn get_initial_concurrency(&self, files: &[FileInfo]) -> usize {
        if let Some(controller) = &self.concurrency_controller {
            // Use adaptive concurrency
            if files.is_empty() {
                return self.config.base_concurrency;
            }

            // Get average file category
            let avg_category = files
                .first()
                .map(|f| f.category)
                .unwrap_or(FileSizeCategory::Medium);

            controller.get_concurrency(avg_category).await
        } else {
            self.config.base_concurrency
        }
    }

    /// Process a single file using a pooled pipeline
    async fn process_single_file(
        pool: Arc<PipelinePool>,
        file_info: FileInfo,
        _algorithms: Vec<HashAlgorithm>,
        progress: Arc<dyn ProgressProvider>,
    ) -> Result<FileProcessingResult> {
        let start_time = Instant::now();

        // Acquire pipeline from pool
        let mut pipeline = pool.acquire().await?;

        // Report initial progress
        progress.report(crate::progress::ProgressUpdate::FileProgress {
            path: file_info.path.clone(),
            bytes_processed: 0,
            total_bytes: file_info.size,
            operation: "Hashing".to_string(),
            throughput_mbps: None,
            memory_usage_bytes: None,
            buffer_size: None,
        });

        // Attach per-file progress to hashing stage (index 1)
        if let Some(stage) = pipeline.stage_mut(1)
            && let Some(hashing) = stage
                .as_any_mut()
                .and_then(|any| any.downcast_mut::<HashingStage>())
        {
            hashing.set_provider(progress.clone());
        }

        // Process the file
        let result = pipeline.process_file(&file_info.path).await;

        // Extract hashes from the hashing stage (at index 1 after validation stage)
        let hashes = match &result {
            Ok(_stats) => {
                if let Some(hashing_stage) = pipeline.stage_mut(1) {
                    if let Some(hashing) = hashing_stage
                        .as_any_mut()
                        .and_then(|any| any.downcast_mut::<HashingStage>())
                    {
                        hashing.take_results().unwrap_or_default()
                    } else {
                        std::collections::HashMap::new()
                    }
                } else {
                    std::collections::HashMap::new()
                }
            }
            Err(_) => std::collections::HashMap::new(),
        };

        // Report completion to any external listeners
        progress.complete();

        // Release pipeline back to pool
        pool.release(pipeline).await;

        // Build result
        match result {
            Ok(stats) => {
                progress.report(crate::progress::ProgressUpdate::FileProgress {
                    path: file_info.path.clone(),
                    bytes_processed: stats.bytes_processed,
                    total_bytes: file_info.size,
                    operation: "Hashing".to_string(),
                    throughput_mbps: Some(stats.throughput_mbps),
                    memory_usage_bytes: None,
                    buffer_size: None,
                });

                Ok(FileProcessingResult {
                    file_path: file_info.path,
                    file_size: file_info.size,
                    hashes,
                    status: ProcessingStatus::Completed,
                    processing_time: start_time.elapsed(),
                })
            }
            Err(_e) => Ok(FileProcessingResult {
                file_path: file_info.path,
                file_size: file_info.size,
                hashes: std::collections::HashMap::new(),
                status: ProcessingStatus::Failed,
                processing_time: start_time.elapsed(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryConfig;
    use crate::progress::NullProvider;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_batch_processor_basic() {
        let temp_dir = TempDir::new().unwrap();
        let mut files = Vec::new();

        // Create test files
        for i in 0..5 {
            let path = temp_dir.path().join(format!("file_{i}.txt"));
            tokio::fs::write(&path, format!("content {i}"))
                .await
                .unwrap();
            files.push(path);
        }

        let config = BatchProcessorConfig::default();
        let client_config = ClientConfig::test();
        let memory_manager = Arc::new(MemoryManager::with_config(MemoryConfig::default()));

        let processor = BatchProcessor::new(config, client_config, memory_manager);
        let progress = Arc::new(NullProvider);

        let result = processor
            .process_batch(files, &[HashAlgorithm::CRC32], progress)
            .await
            .unwrap();

        assert_eq!(result.total_files, 5);
        assert_eq!(result.successful, 5);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    async fn test_pipeline_pooling() {
        let temp_dir = TempDir::new().unwrap();
        let mut files = Vec::new();

        // Create many small files to test pooling
        for i in 0..20 {
            let path = temp_dir.path().join(format!("small_{i}.txt"));
            tokio::fs::write(&path, format!("small content {i}"))
                .await
                .unwrap();
            files.push(path);
        }

        let config = BatchProcessorConfig {
            enable_pooling: true,
            max_pool_size: 4,
            ..Default::default()
        };

        let client_config = ClientConfig::test();
        let memory_manager = Arc::new(MemoryManager::with_config(MemoryConfig::default()));

        let processor = BatchProcessor::new(config, client_config, memory_manager);
        let progress = Arc::new(NullProvider);

        let start = Instant::now();
        let result = processor
            .process_batch(files, &[HashAlgorithm::CRC32], progress)
            .await
            .unwrap();
        let elapsed = start.elapsed();

        assert_eq!(result.total_files, 20);
        assert_eq!(result.successful, 20);

        // Pooling should make this faster
        println!("Processing 20 files took {elapsed:?} with pooling");
    }

    #[tokio::test]
    async fn test_smart_batching() {
        let temp_dir = TempDir::new().unwrap();
        let mut files = Vec::new();

        // Create files of different sizes
        for i in 0..3 {
            let path = temp_dir.path().join(format!("small_{i}.txt"));
            tokio::fs::write(&path, vec![0u8; 1024]).await.unwrap(); // 1KB
            files.push(path);
        }

        for i in 0..2 {
            let path = temp_dir.path().join(format!("large_{i}.txt"));
            tokio::fs::write(&path, vec![0u8; 1024 * 1024])
                .await
                .unwrap(); // 1MB
            files.push(path);
        }

        let config = BatchProcessorConfig {
            smart_batching: true,
            ..Default::default()
        };

        let client_config = ClientConfig::test();
        let memory_manager = Arc::new(MemoryManager::with_config(MemoryConfig::default()));

        let processor = BatchProcessor::new(config, client_config, memory_manager);
        let progress = Arc::new(NullProvider);

        let result = processor
            .process_batch(files, &[HashAlgorithm::CRC32], progress)
            .await
            .unwrap();

        assert_eq!(result.total_files, 5);
        assert_eq!(result.successful, 5);
    }

    #[tokio::test]
    async fn test_error_resilience() {
        let temp_dir = TempDir::new().unwrap();
        let mut files = Vec::new();

        // Create valid files
        for i in 0..3 {
            let path = temp_dir.path().join(format!("valid_{i}.txt"));
            tokio::fs::write(&path, format!("content {i}"))
                .await
                .unwrap();
            files.push(path);
        }

        // Add non-existent file
        files.push(temp_dir.path().join("nonexistent.txt"));

        // Add more valid files
        for i in 3..5 {
            let path = temp_dir.path().join(format!("valid_{i}.txt"));
            tokio::fs::write(&path, format!("content {i}"))
                .await
                .unwrap();
            files.push(path);
        }

        let config = BatchProcessorConfig {
            continue_on_error: true,
            ..Default::default()
        };

        let client_config = ClientConfig::test();
        let memory_manager = Arc::new(MemoryManager::with_config(MemoryConfig::default()));

        let processor = BatchProcessor::new(config, client_config, memory_manager);
        let progress = Arc::new(NullProvider);

        let result = processor
            .process_batch(files, &[HashAlgorithm::CRC32], progress)
            .await
            .unwrap();

        assert_eq!(result.total_files, 6);
        assert_eq!(result.successful, 5); // 5 valid files
        assert_eq!(result.failed, 1); // 1 non-existent file
    }
}
