//! True parallel hashing implementation with independent algorithm queues
//!
//! This module implements a parallel hashing system where each algorithm
//! processes different chunks simultaneously, allowing fast algorithms to
//! race ahead without waiting for slower ones.

use crate::buffer::{allocate_buffer, release_buffer};
use crate::{
    Error, HashAlgorithm, HashResult, Progress, Result,
    error::{InternalError, IoError},
};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use super::traits::{HashAlgorithmExt, StreamingHasher};

/// Configuration for parallel hashing
pub struct ParallelConfig {
    /// Size of each chunk to read from file
    pub chunk_size: Option<usize>,
    /// Maximum number of chunks to queue per algorithm
    pub queue_depth: Option<usize>,
    /// Whether to use OS threads for hash calculation
    pub use_os_threads: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            chunk_size: Some(64 * 1024), // 64KB default
            queue_depth: Some(20),       // Allow up to 20 chunks queued per algorithm
            use_os_threads: true,        // Use OS threads for true parallelism
        }
    }
}

/// Data for a file chunk to be processed
#[derive(Clone)]
pub struct ChunkData {
    /// The chunk data
    pub data: Vec<u8>,
    /// Sequence number for ordering
    pub sequence: u64,
    /// Whether this is the last chunk
    pub is_last: bool,
}

/// Worker for processing chunks for a specific algorithm
struct HashWorker {
    algorithm: HashAlgorithm,
    receiver: mpsc::Receiver<ChunkData>,
    result_tx: mpsc::Sender<(HashAlgorithm, Result<String>)>,
}

impl HashWorker {
    /// Run the worker to process chunks
    async fn run(mut self) {
        let algo_impl = self.algorithm.to_impl();
        let mut hasher: Box<dyn StreamingHasher> = algo_impl.create_hasher();

        // Track chunks for ordering
        let mut pending_chunks: HashMap<u64, Vec<u8>> = HashMap::new();
        let mut next_expected = 0u64;
        let mut last_chunk_received = false;

        // Process chunks as they arrive
        while let Some(chunk) = self.receiver.recv().await {
            if chunk.is_last {
                last_chunk_received = true;
            }

            // Handle chunk ordering
            if chunk.sequence == next_expected {
                // Process this chunk and any pending ones
                hasher.update(&chunk.data);
                next_expected += 1;

                // Process any buffered chunks that are now in sequence
                while let Some(data) = pending_chunks.remove(&next_expected) {
                    hasher.update(&data);
                    next_expected += 1;
                }
            } else {
                // Buffer out-of-order chunk
                pending_chunks.insert(chunk.sequence, chunk.data);
            }

            // If we've received the last chunk and processed all chunks, finalize
            if last_chunk_received && pending_chunks.is_empty() {
                break;
            }
        }

        // Ensure all chunks were processed
        if !pending_chunks.is_empty() {
            let _ = self
                .result_tx
                .send((
                    self.algorithm,
                    Err(Error::Internal(InternalError::hash_calculation(
                        &self.algorithm.to_string(),
                        "Missing chunks in sequence",
                    ))),
                ))
                .await;
            return;
        }

        // Finalize and send result
        let hash = hasher.finalize();
        let _ = self.result_tx.send((self.algorithm, Ok(hash))).await;
    }

    /// Run the worker in an OS thread for true parallelism
    fn run_in_thread(self) {
        std::thread::spawn(move || {
            // Create a single-threaded runtime for this worker
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(self.run());
        });
    }
}

/// Calculate multiple hashes in true parallel with independent queues
#[allow(dead_code)]
pub async fn calculate_parallel_independent(
    file_path: &Path,
    algorithms: &[HashAlgorithm],
    config: ParallelConfig,
    progress_tx: mpsc::Sender<Progress>,
) -> Result<HashMap<HashAlgorithm, HashResult>> {
    let start_time = Instant::now();

    if algorithms.is_empty() {
        return Ok(HashMap::new());
    }

    if !file_path.exists() {
        return Err(Error::Io(IoError::file_not_found(file_path)));
    }

    // Open file and get metadata
    let mut file = File::open(file_path).await?;
    let metadata = file.metadata().await?;
    let file_size = metadata.len();

    // Determine chunk size - use ED2K chunk size if ED2K is included
    let chunk_size = if algorithms.contains(&HashAlgorithm::ED2K) {
        9728000 // ED2K chunk size (9.5MB)
    } else {
        config.chunk_size.unwrap_or(64 * 1024) // Default to 64KB
    };

    // Create channels for each algorithm with bounded capacity
    let mut senders: HashMap<HashAlgorithm, mpsc::Sender<ChunkData>> = HashMap::new();
    let mut workers = Vec::new();

    // Channel for collecting results
    let (result_tx, mut result_rx) =
        mpsc::channel::<(HashAlgorithm, Result<String>)>(algorithms.len());

    // Create a worker for each algorithm
    for &algorithm in algorithms {
        let (tx, rx) = mpsc::channel::<ChunkData>(config.queue_depth.unwrap_or(20));
        senders.insert(algorithm, tx);

        let worker = HashWorker {
            algorithm,
            receiver: rx,
            result_tx: result_tx.clone(),
        };

        if config.use_os_threads {
            // Run in OS thread for true parallelism
            worker.run_in_thread();
        } else {
            // Run as async task
            workers.push(tokio::spawn(worker.run()));
        }
    }

    // Drop original result_tx so result_rx closes when all workers finish
    drop(result_tx);

    // Calculate memory usage
    let memory_per_algorithm = chunk_size * config.queue_depth.unwrap_or(20);
    let total_memory = memory_per_algorithm * algorithms.len();

    // Send initial progress
    let _ = progress_tx
        .send(Progress {
            percentage: 0.0,
            bytes_processed: 0,
            total_bytes: file_size,
            throughput_mbps: 0.0,
            current_operation: format!(
                "Calculating {} hashes in parallel (true independent queues) for {}",
                algorithms.len(),
                file_path.display()
            ),
            memory_usage_bytes: Some(total_memory as u64),
            peak_memory_bytes: Some(total_memory as u64),
            buffer_size: Some(chunk_size),
        })
        .await;

    // Clone what we need for the reader task
    let progress_tx_clone = progress_tx.clone();
    let senders_clone = senders.clone();
    let algorithm_count = algorithms.len();

    // Start the file reader task
    let reader_handle = tokio::spawn(async move {
        let mut sequence = 0;
        let mut bytes_processed = 0u64;
        let mut buffer = allocate_buffer(chunk_size)?;

        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                // Send last chunk marker to all algorithms
                for (algorithm, sender) in &senders_clone {
                    let chunk = ChunkData {
                        sequence,
                        data: vec![], // Empty data for last chunk
                        is_last: true,
                    };

                    // Try to send, but don't block if queue is full
                    if sender.send(chunk).await.is_err() {
                        eprintln!("Failed to send last chunk to {algorithm}");
                    }
                }
                break;
            }

            // Create chunk data
            let chunk_data = buffer[..n].to_vec();

            // Distribute chunk to all algorithms
            let mut send_failures = 0;
            for sender in senders_clone.values() {
                let chunk = ChunkData {
                    sequence,
                    data: chunk_data.clone(),
                    is_last: false,
                };

                // Try to send without blocking
                match sender.try_send(chunk) {
                    Ok(_) => {
                        // Successfully sent
                    }
                    Err(mpsc::error::TrySendError::Full(chunk)) => {
                        // Queue is full, this algorithm is slower
                        // Use blocking send for this one
                        if sender.send(chunk).await.is_err() {
                            send_failures += 1;
                        }
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        // Receiver dropped, worker failed
                        send_failures += 1;
                    }
                }
            }

            // If all algorithms failed, stop reading
            if send_failures == senders_clone.len() {
                return Err(Error::Internal(InternalError::hash_calculation(
                    "parallel",
                    "All hash workers failed",
                )));
            }

            sequence += 1;
            bytes_processed += n as u64;

            // Send progress update
            let percentage = (bytes_processed as f64 / file_size as f64) * 100.0;
            let elapsed = start_time.elapsed().as_secs_f64();
            let throughput_mbps = if elapsed > 0.0 {
                (bytes_processed as f64 / (1024.0 * 1024.0)) / elapsed
            } else {
                0.0
            };

            let _ = progress_tx_clone
                .send(Progress {
                    percentage,
                    bytes_processed,
                    total_bytes: file_size,
                    throughput_mbps,
                    current_operation: format!(
                        "Reading chunk {sequence} ({algorithm_count} algorithms processing independently)"
                    ),
                    memory_usage_bytes: Some(total_memory as u64),
                    peak_memory_bytes: Some(total_memory as u64),
                    buffer_size: Some(chunk_size),
                })
                .await;
        }

        release_buffer(buffer);
        Ok::<u64, Error>(bytes_processed)
    });

    // Drop senders to signal EOF to workers
    drop(senders);

    // Collect results from workers
    let mut results = HashMap::new();
    let mut errors = Vec::new();

    while let Some((algorithm, result)) = result_rx.recv().await {
        match result {
            Ok(hash) => {
                results.insert(
                    algorithm,
                    HashResult {
                        algorithm,
                        hash,
                        input_size: file_size,
                        duration: start_time.elapsed(),
                    },
                );
            }
            Err(e) => {
                errors.push((algorithm, e));
            }
        }
    }

    // Wait for reader to finish
    let reader_result = reader_handle.await.map_err(|e| {
        Error::Internal(InternalError::hash_calculation(
            "parallel",
            &format!("Reader task panicked: {e}"),
        ))
    })?;

    let bytes_read = reader_result?;

    // Wait for async workers if not using threads
    if !config.use_os_threads {
        for worker in workers {
            let _ = worker.await;
        }
    }

    // Check for errors
    if !errors.is_empty() {
        return Err(errors.into_iter().next().unwrap().1);
    }

    // Send final progress
    let _ = progress_tx
        .send(Progress {
            percentage: 100.0,
            bytes_processed: bytes_read,
            total_bytes: file_size,
            throughput_mbps: (bytes_read as f64 / (1024.0 * 1024.0))
                / start_time.elapsed().as_secs_f64(),
            current_operation: format!(
                "Completed {} hashes in parallel (independent processing)",
                algorithms.len()
            ),
            memory_usage_bytes: Some(0),
            peak_memory_bytes: Some(total_memory as u64),
            buffer_size: Some(chunk_size),
        })
        .await;

    Ok(results)
}

// Legacy functions removed - replaced by strategy pattern implementation

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_parallel_config_default() {
        let config = ParallelConfig::default();
        assert_eq!(config.chunk_size, Some(64 * 1024));
        assert_eq!(config.queue_depth, Some(20));
        assert!(config.use_os_threads);
    }
}
