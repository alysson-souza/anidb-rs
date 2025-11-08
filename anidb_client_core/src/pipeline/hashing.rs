//! Hashing stage for the streaming pipeline
//!
//! This stage calculates hashes for data chunks as they flow through the pipeline.

use super::ProcessingStage;
use crate::Result;
use crate::hashing::{HashAlgorithm, HashAlgorithmExt, StreamingHasher};
use crate::progress::{ProgressProvider, ProgressUpdate};
use async_trait::async_trait;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Wrapper for StreamingHasher to make it Sync
struct HasherWrapper {
    hasher: Mutex<Box<dyn StreamingHasher>>,
}

impl std::fmt::Debug for HasherWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HasherWrapper").finish()
    }
}

/// Stage that calculates hashes for streaming data
pub struct HashingStage {
    /// Map of algorithm to its streaming hasher (wrapped for thread-safety)
    hashers: Arc<HashMap<HashAlgorithm, HasherWrapper>>,
    /// Hash results after finalization
    results: Arc<Mutex<Option<HashMap<HashAlgorithm, String>>>>,
    /// Optional progress provider for emitting HashProgress updates
    progress: Option<Arc<dyn ProgressProvider>>,
    /// Total size to process (for progress)
    total_size: u64,
    /// Bytes processed so far (for progress)
    bytes_processed: u64,
    /// Optional parallel workers for multi-algorithm acceleration
    parallel: Option<ParallelState>,
}

impl HashingStage {
    /// Create a new hashing stage with the specified algorithms
    pub fn new(algorithms: &[HashAlgorithm]) -> Self {
        let mut hashers = HashMap::new();

        for &algorithm in algorithms {
            let impl_arc = algorithm.to_impl();
            let hasher = impl_arc.create_hasher();
            hashers.insert(
                algorithm,
                HasherWrapper {
                    hasher: Mutex::new(hasher),
                },
            );
        }

        Self {
            hashers: Arc::new(hashers),
            results: Arc::new(Mutex::new(None)),
            progress: None,
            total_size: 0,
            bytes_processed: 0,
            parallel: None,
        }
    }

    /// Create a new hashing stage with a progress provider
    pub fn new_with_progress(
        algorithms: &[HashAlgorithm],
        provider: Arc<dyn ProgressProvider>,
    ) -> Self {
        let mut stage = Self::new(algorithms);
        stage.progress = Some(provider);
        stage
    }

    /// Set a progress provider for this stage
    pub fn set_progress_provider(&mut self, provider: Option<Arc<dyn ProgressProvider>>) {
        self.progress = provider;
    }

    /// Set a progress provider (non-optional convenience)
    pub fn set_provider(&mut self, provider: Arc<dyn ProgressProvider>) {
        self.progress = Some(provider);
    }

    /// Helper to emit a progress update
    fn emit_progress(&self) {
        if let Some(provider) = &self.progress {
            provider.report(ProgressUpdate::HashProgress {
                algorithm: if self.hashers.len() == 1 {
                    // Single algorithm name for clarity
                    self.hashers
                        .keys()
                        .next()
                        .map(|a| format!("{a:?}"))
                        .unwrap_or_else(|| "Hash".to_string())
                } else {
                    format!("Multiple ({} algorithms)", self.hashers.len())
                },
                bytes_processed: self.bytes_processed,
                total_bytes: self.total_size,
            });
        }
    }

    /// Setup parallel workers when multiple algorithms are requested
    fn setup_parallel_workers(&mut self) {
        if self.hashers.len() <= 1 {
            self.parallel = None;
            return;
        }

        let mut txs = HashMap::new();
        let mut handles = Vec::new();

        for (&algorithm, _) in self.hashers.iter() {
            // Bounded queue to enforce backpressure and keep progress accurate
            let (tx, mut rx) = mpsc::channel::<ChunkMsg>(2);
            txs.insert(algorithm, tx);

            // Spawn OS thread worker similar to parallel strategy
            let handle = std::thread::spawn(move || {
                let algo_impl = algorithm.to_impl();
                let mut hasher = algo_impl.create_hasher();

                let _rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                loop {
                    let msg = _rt.block_on(rx.recv());
                    match msg {
                        Some(ChunkMsg::Data(buf)) => {
                            hasher.update(&buf);
                        }
                        Some(ChunkMsg::End) => break,
                        None => break,
                    }
                }

                (algorithm, hasher.finalize())
            });

            handles.push(handle);
        }

        self.parallel = Some(ParallelState { txs, handles });
    }

    // grouped/seq modes removed in simplification

    // sequential multi-hasher mode removed in simplification

    /// Choose the hashing mode for multi-algorithm cases
    fn choose_mode(&mut self) {
        if self.hashers.len() <= 1 {
            self.parallel = None;
            return;
        }
        self.setup_parallel_workers();
    }

    /// Get the hash results after finalization
    pub fn results(&self) -> Option<HashMap<HashAlgorithm, String>> {
        self.results.lock().unwrap().clone()
    }

    /// Take ownership of the hash results
    pub fn take_results(&mut self) -> Option<HashMap<HashAlgorithm, String>> {
        self.results.lock().unwrap().take()
    }
}

impl fmt::Debug for HashingStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let has_results = self.results.lock().map(|r| r.is_some()).unwrap_or(false);
        f.debug_struct("HashingStage")
            .field("algorithms", &self.hashers.keys().collect::<Vec<_>>())
            .field("has_results", &has_results)
            .field("has_progress", &self.progress.is_some())
            .field("total_size", &self.total_size)
            .field("bytes_processed", &self.bytes_processed)
            .finish()
    }
}

/// Message type for parallel hashing workers
enum ChunkMsg {
    Data(Arc<Vec<u8>>),
    End,
}

struct ParallelState {
    txs: HashMap<HashAlgorithm, mpsc::Sender<ChunkMsg>>,
    handles: Vec<std::thread::JoinHandle<(HashAlgorithm, String)>>,
}

#[async_trait]
impl ProcessingStage for HashingStage {
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        if let Some(p) = &mut self.parallel {
            // Broadcast chunk to workers using shared buffer
            let shared = Arc::new(chunk.to_vec());
            for tx in p.txs.values_mut() {
                // Try non-blocking; on full queue, await to apply backpressure
                if tx.try_send(ChunkMsg::Data(shared.clone())).is_err() {
                    let _ = tx.send(ChunkMsg::Data(shared.clone())).await;
                }
            }
        } else {
            // Sequential update of all hashers
            for wrapper in self.hashers.values() {
                let mut hasher = wrapper.hasher.lock().unwrap();
                hasher.update(chunk);
            }
        }
        // Update and emit progress
        self.bytes_processed += chunk.len() as u64;
        self.emit_progress();
        Ok(())
    }

    async fn initialize(&mut self, _total_size: u64) -> Result<()> {
        // Reset all hashers by recreating them
        let mut new_hashers = HashMap::new();
        for (&algorithm, _) in self.hashers.iter() {
            let impl_arc = algorithm.to_impl();
            let hasher = impl_arc.create_hasher();
            new_hashers.insert(
                algorithm,
                HasherWrapper {
                    hasher: Mutex::new(hasher),
                },
            );
        }
        self.hashers = Arc::new(new_hashers);
        *self.results.lock().unwrap() = None;
        // Capture total size and emit initial progress
        self.total_size = _total_size;
        self.bytes_processed = 0;
        // Choose hashing mode for multi-algorithm runs
        self.choose_mode();
        self.emit_progress();
        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        let mut results = HashMap::new();

        if let Some(mut p) = self.parallel.take() {
            // Signal end to all workers
            for tx in p.txs.values_mut() {
                let _ = tx.send(ChunkMsg::End).await;
            }
            // Join workers and collect results
            for handle in p.handles.drain(..) {
                if let Ok((algo, hash)) = handle.join() {
                    results.insert(algo, hash);
                }
            }
        } else {
            // Finalize all hashers and collect results (sequential path)
            // We need to recreate hashers after finalization since finalize consumes the hasher
            let mut new_hashers = HashMap::new();

            for (&algorithm, wrapper) in self.hashers.iter() {
                // Take the hasher from the wrapper to finalize it
                let impl_arc = algorithm.to_impl();
                let hasher = {
                    let mut guard = wrapper.hasher.lock().unwrap();
                    // Replace with a new hasher and take the old one
                    std::mem::replace(&mut *guard, impl_arc.create_hasher())
                };

                // Finalize the old hasher
                let hash = hasher.finalize();
                results.insert(algorithm, hash);

                // Create a fresh hasher for future use
                new_hashers.insert(
                    algorithm,
                    HasherWrapper {
                        hasher: Mutex::new(impl_arc.create_hasher()),
                    },
                );
            }

            self.hashers = Arc::new(new_hashers);
        }

        *self.results.lock().unwrap() = Some(results);
        // Ensure a final progress update at completion
        self.bytes_processed = self.total_size;
        self.emit_progress();
        Ok(())
    }

    fn name(&self) -> &str {
        "HashingStage"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// Builder for HashingStage with additional configuration
#[allow(dead_code)]
pub struct HashingStageBuilder {
    algorithms: Vec<HashAlgorithm>,
}

#[allow(dead_code)]
impl HashingStageBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            algorithms: Vec::new(),
        }
    }

    /// Add an algorithm to calculate
    pub fn with_algorithm(mut self, algorithm: HashAlgorithm) -> Self {
        if !self.algorithms.contains(&algorithm) {
            self.algorithms.push(algorithm);
        }
        self
    }

    /// Add multiple algorithms
    pub fn with_algorithms(mut self, algorithms: &[HashAlgorithm]) -> Self {
        for &algo in algorithms {
            if !self.algorithms.contains(&algo) {
                self.algorithms.push(algo);
            }
        }
        self
    }

    /// Build the hashing stage
    pub fn build(self) -> HashingStage {
        HashingStage::new(&self.algorithms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hashing_stage_single_algorithm() {
        let mut stage = HashingStage::new(&[HashAlgorithm::CRC32]);

        // Initialize
        stage.initialize(100).await.unwrap();

        // Process some data
        stage.process(b"Hello").await.unwrap();
        stage.process(b"World").await.unwrap();

        // Finalize
        stage.finalize().await.unwrap();

        // Check results
        let results = stage.results().unwrap();
        assert_eq!(results.len(), 1);
        assert!(results.contains_key(&HashAlgorithm::CRC32));
    }

    #[tokio::test]
    async fn test_hashing_stage_multiple_algorithms() {
        let algorithms = vec![HashAlgorithm::CRC32, HashAlgorithm::MD5];
        let mut stage = HashingStage::new(&algorithms);

        // Initialize
        stage.initialize(100).await.unwrap();

        // Process data
        stage.process(b"Test data").await.unwrap();

        // Finalize
        stage.finalize().await.unwrap();

        // Check results
        let results = stage.results().unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains_key(&HashAlgorithm::CRC32));
        assert!(results.contains_key(&HashAlgorithm::MD5));
    }

    #[test]
    fn test_builder() {
        let stage = HashingStageBuilder::new()
            .with_algorithm(HashAlgorithm::ED2K)
            .with_algorithms(&[HashAlgorithm::CRC32, HashAlgorithm::MD5])
            .build();

        // Check that we have 3 algorithms configured
        assert_eq!(stage.hashers.len(), 3);
    }
}
