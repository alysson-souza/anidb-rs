//! Hash calculation strategies for optimized file processing
//!
//! This module provides different strategies for calculating file hashes,
//! each optimized for specific scenarios:
//!
//! - `SequentialStrategy`: Simple, memory-efficient single-threaded processing
//! - `MultipleStrategy`: Calculate multiple hashes in a single file pass
//! - `ParallelStrategy`: True parallel processing with broadcast architecture
//! - `HybridStrategy`: Ring buffer approach for balanced performance
//!
//! The appropriate strategy is automatically selected based on file size,
//! algorithm requirements, and system resources.

use crate::progress::{NullProvider, ProgressProvider};
use crate::{HashAlgorithm, HashResult, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

// Strategy implementations
mod hybrid;
mod multiple;
mod parallel;
mod selector;
mod sequential;

// Re-export public types
pub use hybrid::HybridStrategy;
pub use multiple::MultipleStrategy;
pub use parallel::ParallelStrategy;
pub use selector::{StrategyHint, StrategySelector};
pub use sequential::SequentialStrategy;

/// Context information passed to hashing strategies
#[derive(Debug, Clone)]
pub struct HashingContext {
    /// Path to the file being hashed
    pub file_path: PathBuf,
    /// Size of the file in bytes
    pub file_size: u64,
    /// Hash algorithms to calculate
    pub algorithms: Vec<HashAlgorithm>,
    /// Configuration parameters
    pub config: HashConfig,
}

/// Configuration for hash calculation strategies
#[derive(Debug, Clone)]
pub struct HashConfig {
    /// Buffer size for I/O operations
    pub buffer_size: usize,
    /// Chunk size for parallel processing
    pub chunk_size: usize,
    /// Number of parallel workers
    pub parallel_workers: usize,
    /// Whether to use memory-mapped I/O when available
    pub use_mmap: bool,
    /// ED2K hash variant to use
    pub ed2k_variant: crate::hashing::Ed2kVariant,
}

impl Default for HashConfig {
    fn default() -> Self {
        Self {
            buffer_size: 64 * 1024,                         // 64KB default buffer
            chunk_size: 9728000,                            // ED2K chunk size
            parallel_workers: 4,                            // 4 workers by default
            use_mmap: false,                                // Disabled by default for safety
            ed2k_variant: crate::hashing::Ed2kVariant::Red, // AniDB-compatible
        }
    }
}

/// Memory requirements for a strategy
#[derive(Debug, Clone)]
pub struct MemoryRequirements {
    /// Minimum memory needed in bytes
    pub minimum: usize,
    /// Optimal memory for best performance
    pub optimal: usize,
    /// Maximum memory that can be utilized
    pub maximum: usize,
}

/// Performance metrics from strategy execution
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Total duration of the operation
    pub duration: Duration,
    /// Average throughput in MB/s
    pub throughput_mbps: f64,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: u64,
    /// Number of I/O operations performed
    pub io_operations: u64,
}

/// Result from strategy execution
#[derive(Debug)]
pub struct StrategyResult {
    /// Hash results for each algorithm
    pub results: HashMap<HashAlgorithm, HashResult>,
    /// Performance metrics from execution
    pub metrics: PerformanceMetrics,
}

/// Core trait for hash calculation strategies
#[async_trait]
pub trait HashingStrategy: Send + Sync {
    /// Strategy identifier for logging and metrics
    fn name(&self) -> &'static str;

    /// Calculate memory requirements for this strategy
    fn memory_requirements(&self, file_size: u64) -> MemoryRequirements;

    /// Execute the hashing strategy without progress reporting
    async fn execute(&self, context: HashingContext) -> Result<StrategyResult> {
        // Default implementation delegates to execute_with_progress with NullProvider
        self.execute_with_progress(context, &NullProvider).await
    }

    /// Execute the hashing strategy with progress reporting
    async fn execute_with_progress(
        &self,
        context: HashingContext,
        progress_provider: &dyn ProgressProvider,
    ) -> Result<StrategyResult>;

    /// Check if this strategy is suitable for the given context
    fn is_suitable(&self, context: &HashingContext) -> bool;

    /// Priority score for this strategy (higher is better)
    /// Used when multiple strategies are suitable
    fn priority_score(&self, context: &HashingContext) -> u32 {
        // Default scoring based on common heuristics
        let mut score = 100;

        // Prefer strategies that handle multiple algorithms well
        if context.algorithms.len() > 1 {
            score += 10 * context.algorithms.len() as u32;
        }

        // Adjust for file size
        if context.file_size < 10 * 1024 * 1024 {
            // Small files: prefer simple strategies
            score += 50;
        } else if context.file_size > 1024 * 1024 * 1024 {
            // Large files: prefer parallel strategies
            score += 20;
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config: HashConfig = Default::default();
        assert_eq!(config.buffer_size, 64 * 1024);
        assert_eq!(config.chunk_size, 9728000);
        assert_eq!(config.parallel_workers, 4);
        assert!(!config.use_mmap);
    }

    #[test]
    fn test_memory_requirements() {
        let req = MemoryRequirements {
            minimum: 1024,
            optimal: 4096,
            maximum: 8192,
        };
        assert!(req.minimum <= req.optimal);
        assert!(req.optimal <= req.maximum);
    }
}
