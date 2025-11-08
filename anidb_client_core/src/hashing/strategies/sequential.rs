//! Sequential hash calculation strategy
//!
//! Simple, memory-efficient strategy that processes files sequentially
//! with a single buffer. Best for single algorithms and small files.

use super::{
    HashingContext, HashingStrategy, MemoryRequirements, PerformanceMetrics, StrategyResult,
};
use crate::buffer::{allocate_buffer, release_buffer};
use crate::hashing::{HashAlgorithmExt, HashResult, StreamingHasher};
use crate::progress::{ProgressProvider, ProgressUpdate};
use crate::{
    Error, Result,
    error::{IoError, ValidationError},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// Sequential strategy - simple, memory-efficient single-threaded processing
pub struct SequentialStrategy {
    buffer_size: usize,
}

impl SequentialStrategy {
    /// Create a new sequential strategy with the specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    /// Create with default buffer size
    pub fn default() -> Self {
        Self::new(64 * 1024) // 64KB default
    }
}

#[async_trait]
impl HashingStrategy for SequentialStrategy {
    fn name(&self) -> &'static str {
        "sequential"
    }

    fn memory_requirements(&self, _file_size: u64) -> MemoryRequirements {
        // Sequential needs minimal memory: just one buffer + hasher state
        let hasher_overhead = 1024; // Approximate hasher state size
        MemoryRequirements {
            minimum: self.buffer_size + hasher_overhead,
            optimal: self.buffer_size * 2 + hasher_overhead, // Double buffering would be nice
            maximum: self.buffer_size * 4 + hasher_overhead, // But we don't really need more
        }
    }

    async fn execute_with_progress(
        &self,
        context: HashingContext,
        progress_provider: &dyn ProgressProvider,
    ) -> Result<StrategyResult> {
        let start_time = Instant::now();

        // Validate inputs
        if context.algorithms.is_empty() {
            return Err(Error::Validation(ValidationError::invalid_configuration(
                "No algorithms specified",
            )));
        }

        if !context.file_path.exists() {
            return Err(Error::Io(IoError::file_not_found(&context.file_path)));
        }

        // Open the file
        let mut file = File::open(&context.file_path).await?;
        let mut bytes_processed = 0u64;
        let mut io_operations = 0u64;

        // Use the first algorithm (sequential is best for single algorithm)
        let algorithm = context.algorithms[0];
        let algo_impl = algorithm.to_impl();

        // Allocate buffer
        let mut buffer = allocate_buffer(self.buffer_size)?;

        // Calculate memory usage
        let hasher_state_size = algo_impl.memory_overhead();
        let memory_usage = self.buffer_size + hasher_state_size;
        let mut peak_memory = memory_usage as u64;

        // Create streaming hasher
        let mut hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();

        // Process file in chunks
        loop {
            let n = file.read(&mut buffer).await?;
            io_operations += 1;

            if n == 0 {
                break;
            }

            hasher.update(&buffer[..n]);
            bytes_processed += n as u64;

            // Report progress
            progress_provider.report(ProgressUpdate::HashProgress {
                algorithm: format!("{algorithm:?}"),
                bytes_processed,
                total_bytes: context.file_size,
            });
        }

        // Finalize hash
        let hash = hasher.finalize();

        // Release buffer
        release_buffer(buffer);

        // Calculate final metrics
        let duration = start_time.elapsed();
        let throughput_mbps = if duration.as_secs_f64() > 0.0 {
            (bytes_processed as f64 / 1_048_576.0) / duration.as_secs_f64()
        } else {
            0.0
        };

        // Build results
        let mut results = HashMap::new();
        results.insert(
            algorithm,
            HashResult {
                algorithm,
                hash,
                input_size: context.file_size,
                duration,
            },
        );

        // If multiple algorithms requested, calculate them sequentially
        // (not optimal, but this is the simple strategy)
        for &algo in &context.algorithms[1..] {
            let result = self
                .calculate_single(
                    &context.file_path,
                    algo,
                    context.file_size,
                    &mut peak_memory,
                    &mut io_operations,
                    progress_provider,
                )
                .await?;
            results.insert(algo, result);
        }

        Ok(StrategyResult {
            results,
            metrics: PerformanceMetrics {
                duration: start_time.elapsed(),
                throughput_mbps,
                peak_memory_bytes: peak_memory,
                io_operations,
            },
        })
    }

    fn is_suitable(&self, context: &HashingContext) -> bool {
        // Sequential is suitable for:
        // - Single algorithm
        // - Small files (< 100MB)
        // - When memory is very constrained
        context.algorithms.len() == 1
            || context.file_size < 100 * 1024 * 1024
            || context.config.buffer_size < 32 * 1024
    }

    fn priority_score(&self, context: &HashingContext) -> u32 {
        let mut score: u32 = 100;

        // Strong preference for single algorithm
        if context.algorithms.len() == 1 {
            score += 200;
        }

        // Good for small files
        if context.file_size < 10 * 1024 * 1024 {
            score += 100;
        } else if context.file_size < 100 * 1024 * 1024 {
            score += 50;
        }

        // Penalty for multiple algorithms (we'd have to read the file multiple times)
        if context.algorithms.len() > 1 {
            score = score.saturating_sub(50 * context.algorithms.len() as u32);
        }

        score
    }
}

impl SequentialStrategy {
    /// Helper to calculate a single hash
    async fn calculate_single(
        &self,
        file_path: &std::path::Path,
        algorithm: crate::HashAlgorithm,
        file_size: u64,
        peak_memory: &mut u64,
        io_operations: &mut u64,
        progress_provider: &dyn ProgressProvider,
    ) -> Result<HashResult> {
        let start_time = Instant::now();
        let mut file = File::open(file_path).await?;

        let algo_impl = algorithm.to_impl();
        let mut buffer = allocate_buffer(self.buffer_size)?;

        let memory_usage = self.buffer_size + algo_impl.memory_overhead();
        *peak_memory = (*peak_memory).max(memory_usage as u64);

        let mut hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();
        let mut bytes_processed = 0u64;

        loop {
            let n = file.read(&mut buffer).await?;
            *io_operations += 1;

            if n == 0 {
                break;
            }

            hasher.update(&buffer[..n]);
            bytes_processed += n as u64;

            // Report progress
            progress_provider.report(ProgressUpdate::HashProgress {
                algorithm: format!("{algorithm:?}"),
                bytes_processed,
                total_bytes: file_size,
            });
        }

        let hash = hasher.finalize();
        release_buffer(buffer);

        Ok(HashResult {
            algorithm,
            hash,
            input_size: file_size,
            duration: start_time.elapsed(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::HashAlgorithm;
    use std::path::PathBuf;

    #[test]
    fn test_sequential_strategy_creation() {
        let strategy = SequentialStrategy::new(8192);
        assert_eq!(strategy.name(), "sequential");
        assert_eq!(strategy.buffer_size, 8192);
    }

    #[test]
    fn test_memory_requirements() {
        let strategy = SequentialStrategy::new(64 * 1024);
        let req = strategy.memory_requirements(1024 * 1024);

        assert!(req.minimum <= req.optimal);
        assert!(req.optimal <= req.maximum);
        assert!(req.minimum >= 64 * 1024); // At least buffer size
    }

    #[test]
    fn test_suitability() {
        let strategy = SequentialStrategy::default();

        // Should be suitable for single algorithm
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024, // 1GB
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };
        assert!(strategy.is_suitable(&context));

        // Should be suitable for small files even with multiple algorithms
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 50 * 1024 * 1024, // 50MB
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        assert!(strategy.is_suitable(&context));
    }

    #[test]
    fn test_priority_scoring() {
        let strategy = SequentialStrategy::default();

        // High priority for single algorithm
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 100 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };
        let score1 = strategy.priority_score(&context);

        // Lower priority for multiple algorithms
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 100 * 1024 * 1024,
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
            ],
            config: Default::default(),
        };
        let score2 = strategy.priority_score(&context);

        assert!(score1 > score2);
    }
}
