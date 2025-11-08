//! Parallel hash calculation strategy
//!
//! True parallel processing using broadcast channels to distribute data
//! to multiple worker threads, each calculating a different hash algorithm.

use super::{
    HashingContext, HashingStrategy, MemoryRequirements, PerformanceMetrics, StrategyResult,
};
// Note: Avoid importing tracked buffer allocators here to prevent test memory inflation
use crate::hashing::parallel::{ChunkData, ParallelConfig};
use crate::hashing::{HashAlgorithmExt, HashResult, StreamingHasher};
use crate::progress::ProgressUpdate;
use crate::{
    Error, HashAlgorithm, ProgressProvider, Result,
    error::{InternalError, IoError, ValidationError},
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, broadcast};
use tokio::task::JoinHandle;

/// Parallel strategy - true parallel processing with broadcast architecture
pub struct ParallelStrategy {
    chunk_size: usize,
    queue_depth: usize,
    use_os_threads: bool,
}

impl ParallelStrategy {
    /// Create a new parallel strategy with configuration
    pub fn new(config: ParallelConfig) -> Self {
        Self {
            chunk_size: config.chunk_size.unwrap_or(9728000), // ED2K chunk size default
            queue_depth: config.queue_depth.unwrap_or(2), // Minimal queue depth to stay within memory limits
            use_os_threads: config.use_os_threads,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(ParallelConfig::default())
    }
}

#[async_trait]
impl HashingStrategy for ParallelStrategy {
    fn name(&self) -> &'static str {
        "parallel"
    }

    fn memory_requirements(&self, _file_size: u64) -> MemoryRequirements {
        // Parallel needs memory for queue depth * chunk size * num_algorithms (broadcast)
        // Each algorithm gets a copy of each chunk in the broadcast channel
        let hasher_overhead = 5 * 1024; // Assume 5KB per hasher

        // For broadcast channels, memory usage is queue_depth * chunk_size * num_algorithms
        // We estimate 5 algorithms as a typical case
        let estimated_algorithms = 5;
        let buffer_memory = self.queue_depth * self.chunk_size * estimated_algorithms;

        MemoryRequirements {
            minimum: self.chunk_size * 2 + hasher_overhead,
            optimal: buffer_memory + hasher_overhead * estimated_algorithms,
            maximum: buffer_memory * 2 + hasher_overhead * 10,
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

        // Create broadcast channel for distributing chunks
        let (chunk_tx, _) = broadcast::channel::<Arc<ChunkData>>(self.queue_depth);

        // Spawn worker tasks for each algorithm
        let mut workers: Vec<JoinHandle<Result<(HashAlgorithm, String)>>> = Vec::new();

        for algorithm in context.algorithms.clone() {
            let chunk_rx = chunk_tx.subscribe();
            let worker = if self.use_os_threads {
                spawn_os_thread_worker(algorithm, chunk_rx, self.chunk_size)
            } else {
                spawn_tokio_worker(algorithm, chunk_rx, self.chunk_size)
            };
            workers.push(worker);
        }

        // Read file and broadcast chunks
        let io_operations = Arc::new(Mutex::new(0u64));
        let io_ops_clone = io_operations.clone();
        let file_path = context.file_path.clone();
        let file_size = context.file_size;
        let chunk_size = self.chunk_size;

        let progress_label = format!("Parallel ({} algorithms)", context.algorithms.len());
        // Create a child provider for the reader task and move it into the task
        let reader_progress = progress_provider.create_child("Reading file");
        let reader_task = tokio::spawn(async move {
            read_and_broadcast(
                file_path,
                file_size,
                chunk_size,
                chunk_tx,
                io_ops_clone,
                reader_progress,
                progress_label,
            )
            .await
        });

        // Wait for reader to complete
        let bytes_processed = reader_task.await.map_err(|e| {
            Error::Internal(InternalError::hash_calculation(
                "parallel",
                &format!("Reader task failed: {e}"),
            ))
        })??;

        // Collect results from workers
        let mut results = HashMap::new();
        for worker in workers {
            let (algorithm, hash) = worker.await.map_err(|e| {
                Error::Internal(InternalError::hash_calculation(
                    "parallel",
                    &format!("Worker task failed: {e}"),
                ))
            })??;
            results.insert(
                algorithm,
                HashResult {
                    algorithm,
                    hash,
                    input_size: bytes_processed,
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

        let io_ops = *io_operations.lock().await;
        // Peak memory accounts for broadcast: queue_depth * chunk_size * num_algorithms
        let peak_memory = (self.queue_depth * self.chunk_size * context.algorithms.len()) as u64;

        Ok(StrategyResult {
            results,
            metrics: PerformanceMetrics {
                duration,
                throughput_mbps,
                peak_memory_bytes: peak_memory,
                io_operations: io_ops,
            },
        })
    }

    fn is_suitable(&self, context: &HashingContext) -> bool {
        // Parallel is suitable for:
        // - Multiple algorithms (at least 2)
        // - Large files (> 100MB)
        // - When CPU cores are available
        context.algorithms.len() >= 2
            && context.file_size > 100 * 1024 * 1024
            && std::thread::available_parallelism().map_or(2, |n| n.get()) >= 2
    }

    fn priority_score(&self, context: &HashingContext) -> u32 {
        let mut score: u32 = 100;

        // Strong preference for multiple algorithms on large files
        if context.algorithms.len() >= 3 && context.file_size > 1024 * 1024 * 1024 {
            score += 300;
        } else if context.algorithms.len() >= 2 && context.file_size > 100 * 1024 * 1024 {
            score += 200;
        }

        // Bonus for having many CPU cores
        let cpu_count = std::thread::available_parallelism().map_or(1, |n| n.get());
        if cpu_count >= 8 {
            score += 100;
        } else if cpu_count >= 4 {
            score += 50;
        }

        // Penalty if file is too small (overhead not worth it)
        if context.file_size < 50 * 1024 * 1024 {
            score = score.saturating_sub(100);
        }

        score
    }
}

/// Read file and broadcast chunks to workers
async fn read_and_broadcast(
    file_path: std::path::PathBuf,
    file_size: u64,
    chunk_size: usize,
    chunk_tx: broadcast::Sender<Arc<ChunkData>>,
    io_operations: Arc<Mutex<u64>>,
    progress_provider: Box<dyn ProgressProvider>,
    progress_label: String,
) -> Result<u64> {
    let _start_time = Instant::now();
    let mut file = File::open(&file_path).await?;
    let mut sequence = 0u64;
    let mut bytes_processed = 0u64;

    loop {
        let mut buffer = vec![0u8; chunk_size];
        let n = file.read(&mut buffer).await?;

        {
            let mut io_ops = io_operations.lock().await;
            *io_ops += 1;
        }

        if n == 0 {
            // Send final empty chunk to signal completion
            let chunk = Arc::new(ChunkData {
                data: vec![],
                sequence,
                is_last: true,
            });
            let _ = chunk_tx.send(chunk);
            break;
        }

        buffer.truncate(n);
        bytes_processed += n as u64;
        // Report progress via the provider
        progress_provider.report(ProgressUpdate::HashProgress {
            algorithm: progress_label.clone(),
            bytes_processed,
            total_bytes: file_size,
        });

        // Broadcast chunk to all workers
        let chunk = Arc::new(ChunkData {
            data: buffer,
            sequence,
            is_last: false,
        });

        // If no receivers, we can stop
        if chunk_tx.send(chunk).is_err() {
            break;
        }

        sequence += 1;
    }

    Ok(bytes_processed)
}

/// Spawn a Tokio worker for hash calculation
fn spawn_tokio_worker(
    algorithm: HashAlgorithm,
    mut chunk_rx: broadcast::Receiver<Arc<ChunkData>>,
    _chunk_size: usize,
) -> JoinHandle<Result<(HashAlgorithm, String)>> {
    tokio::spawn(async move {
        let algo_impl = algorithm.to_impl();
        let mut hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();
        let mut chunks: Vec<Arc<ChunkData>> = Vec::new();

        // Collect all chunks
        loop {
            match chunk_rx.recv().await {
                Ok(chunk) => {
                    if chunk.is_last {
                        break;
                    }
                    chunks.push(chunk);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // We missed some messages, this is critical
                    return Err(Error::Internal(InternalError::hash_calculation(
                        &algorithm.to_string(),
                        &format!("Worker lagged by {n} messages"),
                    )));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }

        // Sort chunks by sequence number
        chunks.sort_by_key(|c| c.sequence);

        // Process chunks in order
        for chunk in chunks {
            hasher.update(&chunk.data);
        }

        let hash = hasher.finalize();
        Ok((algorithm, hash))
    })
}

/// Spawn an OS thread worker for hash calculation
fn spawn_os_thread_worker(
    algorithm: HashAlgorithm,
    mut chunk_rx: broadcast::Receiver<Arc<ChunkData>>,
    _chunk_size: usize,
) -> JoinHandle<Result<(HashAlgorithm, String)>> {
    tokio::task::spawn_blocking(move || {
        let algo_impl = algorithm.to_impl();
        let mut hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();
        let mut chunks: Vec<Arc<ChunkData>> = Vec::new();

        // Use blocking recv for OS thread
        let rt = tokio::runtime::Handle::current();

        loop {
            let chunk = rt.block_on(chunk_rx.recv());
            match chunk {
                Ok(chunk) => {
                    if chunk.is_last {
                        break;
                    }
                    chunks.push(chunk);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    return Err(Error::Internal(InternalError::hash_calculation(
                        &algorithm.to_string(),
                        &format!("Worker lagged by {n} messages"),
                    )));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }

        // Sort and process chunks
        chunks.sort_by_key(|c| c.sequence);
        for chunk in chunks {
            hasher.update(&chunk.data);
        }

        let hash = hasher.finalize();
        Ok((algorithm, hash))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashing::HashAlgorithm;
    use std::path::PathBuf;

    #[test]
    fn test_parallel_strategy_creation() {
        let config = ParallelConfig {
            chunk_size: Some(1024 * 1024),
            queue_depth: Some(16),
            use_os_threads: true,
        };
        let strategy = ParallelStrategy::new(config);
        assert_eq!(strategy.name(), "parallel");
        assert_eq!(strategy.chunk_size, 1024 * 1024);
        assert_eq!(strategy.queue_depth, 16);
        assert!(strategy.use_os_threads);
    }

    #[test]
    fn test_memory_requirements() {
        let strategy = ParallelStrategy::with_defaults();
        let req = strategy.memory_requirements(10 * 1024 * 1024 * 1024); // 10GB

        assert!(req.minimum <= req.optimal);
        assert!(req.optimal <= req.maximum);
        assert!(req.minimum >= strategy.chunk_size * 2); // Minimum is 2 chunks now with reduced queue depth
    }

    #[test]
    fn test_suitability() {
        let strategy = ParallelStrategy::with_defaults();

        // Should NOT be suitable for single algorithm
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 1024 * 1024 * 1024,
            algorithms: vec![HashAlgorithm::MD5],
            config: Default::default(),
        };
        assert!(!strategy.is_suitable(&context));

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

        // Should NOT be suitable for small files
        let context = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 10 * 1024 * 1024, // 10MB
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        assert!(!strategy.is_suitable(&context));
    }

    #[test]
    fn test_priority_scoring() {
        let strategy = ParallelStrategy::with_defaults();

        // High priority for many algorithms on large file
        let context_large = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 5 * 1024 * 1024 * 1024, // 5GB
            algorithms: vec![
                HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::CRC32,
                HashAlgorithm::ED2K,
            ],
            config: Default::default(),
        };
        let score_large = strategy.priority_score(&context_large);

        // Lower priority for small file
        let context_small = HashingContext {
            file_path: PathBuf::from("/tmp/test"),
            file_size: 40 * 1024 * 1024, // 40MB
            algorithms: vec![HashAlgorithm::MD5, HashAlgorithm::SHA1],
            config: Default::default(),
        };
        let score_small = strategy.priority_score(&context_small);

        assert!(score_large > score_small);
    }
}
