//! Core streaming pipeline implementation
//!
//! This module provides the main pipeline that composes processing stages.

use super::{PipelineConfig, PipelineStats, ProcessingStage};
use crate::buffer::MemoryTracker;
use crate::memory::{allocate as mem_allocate, release as mem_release};
use crate::{Error, Result};
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};

/// Main streaming pipeline that composes processing stages
#[derive(Debug)]
pub struct StreamingPipeline {
    /// Processing stages in order
    stages: Vec<Box<dyn ProcessingStage>>,
    /// Pipeline configuration
    config: PipelineConfig,
    /// Memory tracker
    #[allow(dead_code)]
    memory_tracker: MemoryTracker,
    /// Statistics
    stats: PipelineStats,
}

impl StreamingPipeline {
    /// Create a new streaming pipeline
    pub fn new(config: PipelineConfig) -> Self {
        let memory_tracker = MemoryTracker::new(config.max_memory);

        Self {
            stages: Vec::new(),
            config,
            memory_tracker,
            stats: PipelineStats {
                bytes_processed: 0,
                chunks_processed: 0,
                total_duration: std::time::Duration::ZERO,
                throughput_mbps: 0.0,
            },
        }
    }

    /// Add a processing stage to the pipeline
    pub fn add_stage(mut self, stage: Box<dyn ProcessingStage>) -> Self {
        self.stages.push(stage);
        self
    }

    /// Process a file through the pipeline
    pub async fn process_file(&mut self, path: &Path) -> Result<PipelineStats> {
        let start_time = Instant::now();

        // Get file metadata
        let metadata = tokio::fs::metadata(path).await?;
        let file_size = metadata.len();

        // Initialize all stages
        for stage in &mut self.stages {
            stage.initialize(file_size).await?;
        }

        // Open file for streaming
        let file = File::open(path).await?;
        let mut reader = BufReader::with_capacity(self.config.chunk_size, file);

        // Reset stats
        self.stats = PipelineStats {
            bytes_processed: 0,
            chunks_processed: 0,
            total_duration: std::time::Duration::ZERO,
            throughput_mbps: 0.0,
        };

        // Process file in chunks
        loop {
            let mut buffer = mem_allocate(self.config.chunk_size)?;
            let bytes_read = reader.read(&mut buffer).await?;

            if bytes_read == 0 {
                mem_release(buffer); // Release unused buffer
                break; // EOF
            }

            let chunk = &buffer[..bytes_read];

            // Process chunk through all stages
            for stage in &mut self.stages {
                stage.process(chunk).await.map_err(|e| {
                    Error::Internal(crate::error::InternalError::Assertion {
                        message: format!("Stage '{}' failed: {}", stage.name(), e),
                    })
                })?
            }

            self.stats.bytes_processed += bytes_read as u64;
            self.stats.chunks_processed += 1;

            // Return buffer to memory manager
            mem_release(buffer);
        }

        // Finalize all stages
        for stage in &mut self.stages {
            stage.finalize().await?;
        }

        // Calculate final stats
        self.stats.total_duration = start_time.elapsed();
        self.stats.throughput_mbps = if self.stats.total_duration.as_secs_f64() > 0.0 {
            (self.stats.bytes_processed as f64 / self.stats.total_duration.as_secs_f64())
                / (1024.0 * 1024.0)
        } else {
            0.0
        };

        Ok(self.stats.clone())
    }

    /// Process raw bytes through the pipeline
    pub async fn process_bytes(&mut self, data: &[u8]) -> Result<PipelineStats> {
        let start_time = Instant::now();
        let total_size = data.len() as u64;

        // Initialize all stages
        for stage in &mut self.stages {
            stage.initialize(total_size).await?;
        }

        // Reset stats
        self.stats = PipelineStats {
            bytes_processed: 0,
            chunks_processed: 0,
            total_duration: std::time::Duration::ZERO,
            throughput_mbps: 0.0,
        };

        // Process data in chunks
        let mut offset = 0;
        while offset < data.len() {
            let chunk_end = (offset + self.config.chunk_size).min(data.len());
            let chunk = &data[offset..chunk_end];

            // Process chunk through all stages
            for stage in &mut self.stages {
                stage.process(chunk).await?;
            }

            self.stats.bytes_processed += chunk.len() as u64;
            self.stats.chunks_processed += 1;
            offset = chunk_end;
        }

        // Finalize all stages
        for stage in &mut self.stages {
            stage.finalize().await?;
        }

        // Calculate final stats
        self.stats.total_duration = start_time.elapsed();
        self.stats.throughput_mbps = if self.stats.total_duration.as_secs_f64() > 0.0 {
            (self.stats.bytes_processed as f64 / self.stats.total_duration.as_secs_f64())
                / (1024.0 * 1024.0)
        } else {
            0.0
        };

        Ok(self.stats.clone())
    }

    /// Get current statistics
    pub fn stats(&self) -> &PipelineStats {
        &self.stats
    }

    /// Get a reference to a specific stage by index
    pub fn stage(&self, index: usize) -> Option<&dyn ProcessingStage> {
        self.stages.get(index).map(|s| s.as_ref())
    }

    /// Get a mutable reference to a specific stage by index
    pub fn stage_mut(&mut self, index: usize) -> Option<&mut dyn ProcessingStage> {
        match self.stages.get_mut(index) {
            Some(stage) => Some(stage.as_mut()),
            None => None,
        }
    }

    /// Get the number of stages in the pipeline
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }
}

/// Builder for StreamingPipeline
#[allow(dead_code)]
pub struct StreamingPipelineBuilder {
    stages: Vec<Box<dyn ProcessingStage>>,
    config: PipelineConfig,
}

#[allow(dead_code)]
impl StreamingPipelineBuilder {
    /// Create a new builder with default config
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
            config: PipelineConfig::default(),
        }
    }

    /// Create a new builder with custom config
    pub fn with_config(config: PipelineConfig) -> Self {
        Self {
            stages: Vec::new(),
            config,
        }
    }

    /// Add a stage to the pipeline
    pub fn add_stage(mut self, stage: Box<dyn ProcessingStage>) -> Self {
        self.stages.push(stage);
        self
    }

    /// Set the chunk size
    pub fn chunk_size(mut self, size: usize) -> Self {
        self.config.chunk_size = size;
        self
    }

    /// Set whether to use parallel stages
    pub fn parallel_stages(mut self, parallel: bool) -> Self {
        self.config.parallel_stages = parallel;
        self
    }

    /// Set maximum memory usage
    pub fn max_memory(mut self, max: usize) -> Self {
        self.config.max_memory = max;
        self
    }

    /// Build the pipeline
    pub fn build(self) -> StreamingPipeline {
        let memory_tracker = MemoryTracker::new(self.config.max_memory);

        StreamingPipeline {
            stages: self.stages,
            config: self.config,
            memory_tracker,
            stats: PipelineStats {
                bytes_processed: 0,
                chunks_processed: 0,
                total_duration: std::time::Duration::ZERO,
                throughput_mbps: 0.0,
            },
        }
    }
}

impl Default for StreamingPipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::HashAlgorithm;
    use crate::pipeline::{HashingStage, ValidationStage};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_pipeline_basic() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.dat");
        std::fs::write(&test_file, b"Hello, World!").unwrap();

        let mut pipeline = StreamingPipelineBuilder::new().chunk_size(5).build();

        let stats = pipeline.process_file(&test_file).await.unwrap();

        assert_eq!(stats.bytes_processed, 13);
        assert_eq!(stats.chunks_processed, 3); // 5 + 5 + 3 bytes
        assert!(stats.throughput_mbps > 0.0);
    }

    #[tokio::test]
    async fn test_pipeline_with_stages() {
        let validation = Box::new(ValidationStage::new());
        let hashing = Box::new(HashingStage::new(&[HashAlgorithm::CRC32]));

        let mut pipeline = StreamingPipelineBuilder::new()
            .add_stage(validation)
            .add_stage(hashing)
            .chunk_size(10)
            .build();

        let data = b"Test data for pipeline processing";
        let stats = pipeline.process_bytes(data).await.unwrap();

        assert_eq!(stats.bytes_processed, data.len() as u64);
        assert!(stats.chunks_processed > 0);
    }

    #[tokio::test]
    async fn test_pipeline_stage_failure() {
        let validation = Box::new(
            ValidationStage::new().with_max_file_size(10), // Very small limit
        );

        let mut pipeline = StreamingPipelineBuilder::new()
            .add_stage(validation)
            .build();

        let data = b"This is too much data for the validation stage";
        let result = pipeline.process_bytes(data).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pipeline_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty.dat");
        std::fs::write(&test_file, b"").unwrap();

        let mut pipeline = StreamingPipelineBuilder::new().build();

        let stats = pipeline.process_file(&test_file).await.unwrap();

        assert_eq!(stats.bytes_processed, 0);
        assert_eq!(stats.chunks_processed, 0);
    }

    #[test]
    fn test_builder_configuration() {
        let pipeline = StreamingPipelineBuilder::new()
            .chunk_size(8192)
            .parallel_stages(true)
            .max_memory(100 * 1024 * 1024)
            .build();

        assert_eq!(pipeline.config.chunk_size, 8192);
        assert!(pipeline.config.parallel_stages);
        assert_eq!(pipeline.config.max_memory, 100 * 1024 * 1024);
    }
}
