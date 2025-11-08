//! Multiple hash calculation strategy
//!
//! Calculates multiple hash algorithms in a single file pass.
//! Optimal for I/O-bound scenarios where reading the file is the bottleneck.

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

/// Multiple strategy - calculate multiple algorithms in a single pass
pub struct MultipleStrategy {
    buffer_size: usize,
}

impl MultipleStrategy {
    /// Create a new multiple strategy with the specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        Self { buffer_size }
    }

    /// Create with default buffer size
    pub fn with_defaults() -> Self {
        Self::new(64 * 1024) // 64KB default
    }
}

#[async_trait]
impl HashingStrategy for MultipleStrategy {
    fn name(&self) -> &'static str {
        "multiple"
    }

    fn memory_requirements(&self, _file_size: u64) -> MemoryRequirements {
        // Need buffer + state for each hasher
        // Assume ~1KB per hasher state on average
        let hasher_overhead_per_algo = 1024;
        let max_algorithms = 5; // Reasonable maximum

        MemoryRequirements {
            minimum: self.buffer_size + hasher_overhead_per_algo,
            optimal: self.buffer_size * 2 + (hasher_overhead_per_algo * 3),
            maximum: self.buffer_size * 2 + (hasher_overhead_per_algo * max_algorithms),
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

        // Allocate buffer
        let mut buffer = allocate_buffer(self.buffer_size)?;

        // Create streaming hashers for all requested algorithms
        let mut hashers: HashMap<crate::HashAlgorithm, Box<dyn StreamingHasher>> = HashMap::new();
        let mut total_hasher_memory = 0usize;

        for &algorithm in &context.algorithms {
            let algo_impl = algorithm.to_impl();
            total_hasher_memory += algo_impl.memory_overhead();
            let hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();
            hashers.insert(algorithm, hasher);
        }

        // Calculate memory usage
        let memory_usage = self.buffer_size + total_hasher_memory;
        let peak_memory = memory_usage as u64;

        // Process file in chunks, updating all hashers
        loop {
            let n = file.read(&mut buffer).await?;
            io_operations += 1;

            if n == 0 {
                break;
            }

            // Update all hashers with the same data
            let chunk = &buffer[..n];
            for hasher in hashers.values_mut() {
                hasher.update(chunk);
            }

            bytes_processed += n as u64;

            // Report progress
            progress_provider.report(ProgressUpdate::HashProgress {
                algorithm: format!("Multiple ({} algorithms)", context.algorithms.len()),
                bytes_processed,
                total_bytes: context.file_size,
            });
        }

        // Release buffer early
        release_buffer(buffer);

        // Finalize all hashes
        let mut results = HashMap::new();
        let duration = start_time.elapsed();

        for (algorithm, hasher) in hashers {
            let hash = hasher.finalize();
            results.insert(
                algorithm,
                HashResult {
                    algorithm,
                    hash,
                    input_size: context.file_size,
                    duration,
                },
            );
        }

        // Calculate final metrics
        let throughput_mbps = if duration.as_secs_f64() > 0.0 {
            (bytes_processed as f64 / 1_048_576.0) / duration.as_secs_f64()
        } else {
            0.0
        };

        Ok(StrategyResult {
            results,
            metrics: PerformanceMetrics {
                duration,
                throughput_mbps,
                peak_memory_bytes: peak_memory,
                io_operations,
            },
        })
    }

    fn is_suitable(&self, context: &HashingContext) -> bool {
        // Multiple strategy is suitable for:
        // - Multiple algorithms (obviously)
        // - Medium-sized files
        // - When I/O is the bottleneck (not CPU)
        context.algorithms.len() > 1 && context.file_size < 10 * 1024 * 1024 * 1024 // < 10GB
    }

    fn priority_score(&self, context: &HashingContext) -> u32 {
        let mut score: u32 = 100;

        // Strong preference when multiple algorithms requested
        if context.algorithms.len() > 1 {
            score += 100 + (20 * context.algorithms.len() as u32);
        }

        // Good for medium files
        if context.file_size > 100 * 1024 * 1024 && context.file_size < 1024 * 1024 * 1024 {
            score += 50;
        }

        // Check if ED2K is involved (might need special handling)
        let has_ed2k = context.algorithms.contains(&crate::HashAlgorithm::ED2K);

        if has_ed2k && context.algorithms.len() > 1 {
            // Slight penalty as ED2K has special chunking requirements
            score = score.saturating_sub(20);
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::HashAlgorithm;
    use std::path::PathBuf;

    #[test]
    fn test_multiple_strategy_creation() {
        let strategy = MultipleStrategy::new(8192);
        assert_eq!(strategy.name(), "multiple");
        assert_eq!(strategy.buffer_size, 8192);
    }

    #[test]
    fn test_memory_requirements() {
        let strategy = MultipleStrategy::new(64 * 1024);
        let req = strategy.memory_requirements(1024 * 1024);

        assert!(req.minimum <= req.optimal);
        assert!(req.optimal <= req.maximum);
        assert!(req.minimum >= 64 * 1024); // At least buffer size
    }

    #[test]
    fn test_suitability() {
        let strategy = MultipleStrategy::with_defaults();

        // Should NOT be suitable for single algorithm
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 100 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };
        assert!(!strategy.is_suitable(&context));

        // Should be suitable for multiple algorithms
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024, // 500MB
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        assert!(strategy.is_suitable(&context));

        // Should NOT be suitable for very large files
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 20 * 1024 * 1024 * 1024, // 20GB
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        assert!(!strategy.is_suitable(&context));
    }

    #[test]
    fn test_priority_scoring() {
        let strategy = MultipleStrategy::with_defaults();

        // Higher priority for multiple algorithms
        let context_multi = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024,
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
            ],
            config: Default::default(),
        };
        let score_multi = strategy.priority_score(&context_multi);

        // Lower priority for single algorithm
        let context_single = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };
        let score_single = strategy.priority_score(&context_single);

        assert!(score_multi > score_single);

        // Test ED2K penalty
        let context_with_ed2k = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::ED2K, HashAlgorithm::MD5],
            config: Default::default(),
        };
        let score_with_ed2k = strategy.priority_score(&context_with_ed2k);

        let context_without_ed2k = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::SHA1, HashAlgorithm::MD5],
            config: Default::default(),
        };
        let score_without_ed2k = strategy.priority_score(&context_without_ed2k);

        // ED2K should have slightly lower priority due to special requirements
        assert!(score_without_ed2k >= score_with_ed2k);
    }
}
