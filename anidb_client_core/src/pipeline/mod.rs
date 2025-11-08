//! Streaming pipeline architecture for file processing
//!
//! This module provides a composable pipeline for processing file data
//! with clear separation between I/O and processing concerns.

use crate::Result;
use async_trait::async_trait;
use std::fmt::Debug;

mod combinators;
mod hashing;
mod progress;
mod streaming;
mod validation;

pub use combinators::{
    BufferingStage, ConditionalStage, ParallelStage, RateLimitedStage, StageExt, TransformStage,
};
pub use hashing::HashingStage;
pub use progress::ProgressStage;
pub use streaming::{StreamingPipeline, StreamingPipelineBuilder};
pub use validation::ValidationStage;

/// Core trait for pipeline processing stages
///
/// Each stage processes chunks of data as they flow through the pipeline.
/// Stages are composable and can be chained together to form complex
/// processing workflows.
#[async_trait]
pub trait ProcessingStage: Send + Sync + Debug {
    /// Process a chunk of data
    ///
    /// # Arguments
    /// * `chunk` - The data chunk to process
    ///
    /// # Returns
    /// Ok(()) if processing succeeded, Error otherwise
    async fn process(&mut self, chunk: &[u8]) -> Result<()>;

    /// Called when processing starts
    ///
    /// This allows stages to perform initialization that requires
    /// knowledge of the total size (e.g., for progress reporting).
    ///
    /// # Arguments
    /// * `total_size` - Total size of the data to be processed
    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        // Default implementation does nothing
        let _ = total_size;
        Ok(())
    }

    /// Called when all data has been processed
    ///
    /// This allows stages to perform finalization (e.g., computing
    /// final hashes, flushing buffers, etc.)
    async fn finalize(&mut self) -> Result<()> {
        // Default implementation does nothing
        Ok(())
    }

    /// Get the name of this stage for debugging
    fn name(&self) -> &str;

    /// Allow downcasting to concrete types
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

/// Configuration for pipeline execution
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Size of chunks to process
    pub chunk_size: usize,
    /// Whether to run stages in parallel (when possible)
    pub parallel_stages: bool,
    /// Maximum memory usage allowed
    pub max_memory: usize,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            chunk_size: 64 * 1024, // 64KB default
            parallel_stages: false,
            max_memory: 500 * 1024 * 1024, // 500MB
        }
    }
}

/// Statistics from pipeline execution
#[derive(Debug, Clone)]
pub struct PipelineStats {
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Number of chunks processed
    pub chunks_processed: usize,
    /// Total processing time
    pub total_duration: std::time::Duration,
    /// Throughput in MB/s
    pub throughput_mbps: f64,
}
