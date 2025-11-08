//! Hybrid hash calculation strategy using ring buffer architecture
//!
//! This strategy uses a lock-free ring buffer to allow different hash algorithms
//! to process data at their own pace. One reader fills the ring while multiple
//! workers consume at different rates.

use super::{
    HashingContext, HashingStrategy, MemoryRequirements, PerformanceMetrics, StrategyResult,
};
use crate::hashing::buffer_ring::{BufferRing, RingReader};
use crate::hashing::{HashAlgorithmExt, HashResult, StreamingHasher};
use crate::progress::{ProgressProvider, ProgressUpdate};
use crate::{
    Error, HashAlgorithm, Result,
    error::{InternalError, IoError, ValidationError},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs::File;
use tokio::task::JoinHandle;

/// Hybrid strategy - ring buffer approach for balanced performance
pub struct HybridStrategy {
    ring_size: usize,
    worker_count: usize,
    chunk_size: usize,
}

impl HybridStrategy {
    /// Create a new hybrid strategy
    pub fn new(ring_size: usize, worker_count: usize) -> Self {
        Self {
            ring_size,
            worker_count,
            chunk_size: 9728000, // ED2K chunk size for compatibility
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(32, 4) // 32 slots, 4 workers
    }

    /// Create optimized for ED2K with other algorithms
    pub fn for_ed2k_combo(num_algorithms: usize) -> Self {
        // Use smaller ring for ED2K's large chunks
        let ring_size = if num_algorithms <= 2 {
            16 // Smaller ring for fewer algorithms
        } else {
            32 // Larger ring for more algorithms
        };

        Self::new(ring_size, num_algorithms)
    }
}

#[async_trait]
impl HashingStrategy for HybridStrategy {
    fn name(&self) -> &'static str {
        "hybrid"
    }

    fn memory_requirements(&self, _file_size: u64) -> MemoryRequirements {
        // Ring buffer needs ring_size * chunk_size memory
        let ring_memory = self.ring_size * self.chunk_size;
        let hasher_overhead = self.worker_count * 1024; // ~1KB per hasher

        MemoryRequirements {
            minimum: self.chunk_size * 4 + hasher_overhead,
            optimal: ring_memory + hasher_overhead,
            maximum: ring_memory * 2 + hasher_overhead * 2,
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

        // Determine chunk size based on algorithms
        let chunk_size = if context.algorithms.contains(&HashAlgorithm::ED2K) {
            9728000 // ED2K chunk size
        } else {
            self.chunk_size.min(1024 * 1024) // Use smaller chunks if no ED2K
        };

        // Create ring buffer
        let ring = Arc::new(BufferRing::new(chunk_size, context.algorithms.len())?);

        // Spawn worker tasks for each algorithm
        let mut workers: Vec<JoinHandle<Result<(HashAlgorithm, String, u64)>>> = Vec::new();

        for algorithm in context.algorithms.clone() {
            let reader = ring.create_reader();
            let worker = spawn_ring_worker(algorithm, reader, chunk_size);
            workers.push(worker);
        }

        // Fill the ring buffer from file
        let file_path = context.file_path.clone();
        let file_size = context.file_size;
        let ring_clone = ring.clone();

        // Create a child progress provider for the file reading task
        let child_provider = progress_provider.create_child("Reading file");
        let filler_task = tokio::spawn(async move {
            fill_ring_buffer(
                file_path,
                file_size,
                chunk_size,
                ring_clone,
                child_provider.as_ref(),
            )
            .await
        });

        // Wait for filler to complete
        let (bytes_processed, io_operations) = filler_task.await.map_err(|e| {
            Error::Internal(InternalError::hash_calculation(
                "hybrid",
                &format!("Filler task failed: {e}"),
            ))
        })??;

        // Collect results from workers
        let mut results = HashMap::new();
        let mut total_bytes_hashed = 0u64;

        for worker in workers {
            let (algorithm, hash, bytes_hashed) = worker.await.map_err(|e| {
                Error::Internal(InternalError::hash_calculation(
                    "hybrid",
                    &format!("Worker task failed: {e}"),
                ))
            })??;
            total_bytes_hashed = total_bytes_hashed.max(bytes_hashed);

            results.insert(
                algorithm,
                HashResult {
                    algorithm,
                    hash,
                    input_size: bytes_hashed,
                    duration: start_time.elapsed(),
                },
            );
        }

        // Calculate metrics
        let duration = start_time.elapsed();
        let throughput_mbps = if duration.as_secs_f64() > 0.0 {
            (bytes_processed as f64 / 1_048_576.0) / duration.as_secs_f64()
        } else {
            0.0
        };

        let peak_memory = (self.ring_size * chunk_size) as u64;

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
        // Hybrid is especially suitable for:
        // - ED2K combined with other algorithms
        // - Large files with multiple algorithms
        // - When algorithms have different processing speeds
        let has_ed2k = context.algorithms.contains(&HashAlgorithm::ED2K);
        let multiple_algos = context.algorithms.len() >= 2;
        let large_file = context.file_size > 100 * 1024 * 1024;

        multiple_algos && (has_ed2k || large_file)
    }

    fn priority_score(&self, context: &HashingContext) -> u32 {
        let mut score = 100;

        // Strong preference when ED2K is combined with others
        let has_ed2k = context.algorithms.contains(&HashAlgorithm::ED2K);
        if has_ed2k && context.algorithms.len() > 1 {
            score += 250; // High priority for ED2K combinations
        }

        // Good for multiple algorithms on large files
        if context.algorithms.len() >= 3 && context.file_size > 500 * 1024 * 1024 {
            score += 150;
        }

        // Bonus for very large files where ring buffer helps
        if context.file_size > 5 * 1024 * 1024 * 1024 {
            score += 100;
        }

        score
    }
}

/// Fill the ring buffer from a file
async fn fill_ring_buffer(
    file_path: std::path::PathBuf,
    file_size: u64,
    _chunk_size: usize,
    ring: Arc<BufferRing>,
    progress_provider: &dyn ProgressProvider,
) -> Result<(u64, u64)> {
    let _start_time = Instant::now();
    let mut file = File::open(&file_path).await?;
    let mut bytes_processed = 0u64;
    let mut io_operations = 0u64;

    loop {
        // Write to next available slot
        match ring.write_next(&mut file).await? {
            Some(n) => {
                bytes_processed += n as u64;
                io_operations += 1;

                // Report progress
                progress_provider.report(ProgressUpdate::HashProgress {
                    algorithm: "Hybrid".to_string(),
                    bytes_processed,
                    total_bytes: file_size,
                });
            }
            None => {
                // End of file
                ring.mark_complete();
                break;
            }
        }
    }

    Ok((bytes_processed, io_operations))
}

/// Spawn a worker that reads from the ring buffer
fn spawn_ring_worker(
    algorithm: HashAlgorithm,
    mut reader: RingReader,
    _chunk_size: usize,
) -> JoinHandle<Result<(HashAlgorithm, String, u64)>> {
    tokio::spawn(async move {
        let algo_impl = algorithm.to_impl();
        let mut hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();
        let mut bytes_hashed = 0u64;

        // Read chunks from ring buffer
        while let Some(chunk) = reader.read_next().await {
            hasher.update(chunk.data());
            bytes_hashed += chunk.data().len() as u64;

            // Mark chunk as consumed by this reader
            chunk.mark_consumed();
        }

        let hash = hasher.finalize();
        Ok((algorithm, hash, bytes_hashed))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::HashAlgorithm;
    use std::path::PathBuf;

    #[test]
    fn test_hybrid_strategy_creation() {
        let strategy = HybridStrategy::new(16, 3);
        assert_eq!(strategy.name(), "hybrid");
        assert_eq!(strategy.ring_size, 16);
        assert_eq!(strategy.worker_count, 3);
    }

    #[test]
    fn test_ed2k_optimized_creation() {
        let strategy = HybridStrategy::for_ed2k_combo(3);
        assert_eq!(strategy.ring_size, 32);
        assert_eq!(strategy.worker_count, 3);

        let strategy = HybridStrategy::for_ed2k_combo(2);
        assert_eq!(strategy.ring_size, 16);
        assert_eq!(strategy.worker_count, 2);
    }

    #[test]
    fn test_memory_requirements() {
        let strategy = HybridStrategy::new(32, 4);
        let req = strategy.memory_requirements(10 * 1024 * 1024 * 1024);

        assert!(req.minimum <= req.optimal);
        assert!(req.optimal <= req.maximum);
        // Ring buffer needs significant memory
        assert!(req.optimal >= strategy.ring_size * strategy.chunk_size);
    }

    #[test]
    fn test_suitability() {
        let strategy = HybridStrategy::with_defaults();

        // Should be highly suitable for ED2K + others
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::ED2K, HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        assert!(strategy.is_suitable(&context));

        // Should be suitable for multiple algorithms on large file
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 500 * 1024 * 1024,
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
            ],
            config: Default::default(),
        };
        assert!(strategy.is_suitable(&context));

        // Should NOT be suitable for single algorithm
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };
        assert!(!strategy.is_suitable(&context));
    }

    #[test]
    fn test_priority_scoring() {
        let strategy = HybridStrategy::with_defaults();

        // Highest priority for ED2K + others
        let context_ed2k = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::ED2K, HashAlgorithm::MD5],
            config: Default::default(),
        };
        let score_ed2k = strategy.priority_score(&context_ed2k);

        // Lower priority without ED2K
        let context_no_ed2k = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        let score_no_ed2k = strategy.priority_score(&context_no_ed2k);

        assert!(score_ed2k > score_no_ed2k);

        // Higher priority for very large files
        let context_huge = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 10 * 1024 * 1024 * 1024, // 10GB
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
            ],
            config: Default::default(),
        };
        let score_huge = strategy.priority_score(&context_huge);

        let context_medium = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 200 * 1024 * 1024, // 200MB
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
            ],
            config: Default::default(),
        };
        let score_medium = strategy.priority_score(&context_medium);

        assert!(score_huge > score_medium);
    }
}
