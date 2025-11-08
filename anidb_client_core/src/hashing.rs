//! Hash calculation functionality for the AniDB Client Core Library
//!
//! This module contains hash algorithm implementations.

use crate::buffer::MemoryTracker;
use crate::progress::ProgressProvider;
use crate::{
    Error, Result,
    error::{InternalError, IoError, ValidationError},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

// New trait system modules
mod algorithms;
mod buffer_ring;
mod parallel;
mod registry;
mod strategies;
mod traits;

// Re-export public types from trait system
pub use parallel::{ChunkData, ParallelConfig};
pub use registry::AlgorithmRegistry;
pub use strategies::{
    HashConfig, HashingStrategy, HybridStrategy, ParallelStrategy, StrategyHint, StrategySelector,
};
pub use traits::{HashAlgorithmExt, HashAlgorithmImpl, StreamingHasher};

/// Hash algorithms supported by the client
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HashAlgorithm {
    /// ED2K hash algorithm
    ED2K,
    /// CRC32 hash algorithm
    CRC32,
    /// MD5 hash algorithm
    MD5,
    /// SHA1 hash algorithm
    SHA1,
    /// Tiger Tree Hash algorithm
    TTH,
}

impl std::fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashAlgorithm::ED2K => write!(f, "ed2k"),
            HashAlgorithm::CRC32 => write!(f, "crc32"),
            HashAlgorithm::MD5 => write!(f, "md5"),
            HashAlgorithm::SHA1 => write!(f, "sha1"),
            HashAlgorithm::TTH => write!(f, "tth"),
        }
    }
}

impl std::str::FromStr for HashAlgorithm {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "ed2k" => Ok(HashAlgorithm::ED2K),
            "crc32" => Ok(HashAlgorithm::CRC32),
            "md5" => Ok(HashAlgorithm::MD5),
            "sha1" => Ok(HashAlgorithm::SHA1),
            "tth" => Ok(HashAlgorithm::TTH),
            _ => Err(Error::Validation(ValidationError::invalid_configuration(
                &format!("Unknown hash algorithm: {s}"),
            ))),
        }
    }
}

impl HashAlgorithmExt for HashAlgorithm {
    fn to_impl(&self) -> Arc<dyn HashAlgorithmImpl> {
        AlgorithmRegistry::global()
            .get(&self.to_string())
            .expect("Algorithm should be registered")
    }
}

/// ED2K hash variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ed2kVariant {
    /// Blue variant: Standard ED2K implementation
    Blue,
    /// Red variant: AniDB-compatible, appends MD4 of empty data when file size is exact multiple of chunk size
    Red,
}

/// Result of hash calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashResult {
    pub algorithm: HashAlgorithm,
    pub hash: String,
    pub input_size: u64,
    pub duration: Duration,
}

/// Hash calculator for file processing
#[derive(Clone)]
pub struct HashCalculator {
    /// Strategy selector for choosing optimal hashing approach
    selector: Arc<StrategySelector>,
    /// Memory tracker for buffer allocation
    memory_tracker: MemoryTracker,
}

impl std::fmt::Debug for HashCalculator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashCalculator")
            .field("selector", &self.selector)
            .field("memory_limit", &self.memory_tracker.limit())
            .field("memory_used", &self.memory_tracker.used())
            .finish()
    }
}

impl HashCalculator {
    /// Create a new hash calculator
    pub fn new() -> Self {
        Self {
            selector: Arc::new(StrategySelector::new()),
            memory_tracker: MemoryTracker::default(),
        }
    }

    /// Create a hash calculator with a specific strategy hint
    pub fn with_hint(hint: StrategyHint) -> Self {
        Self {
            selector: Arc::new(StrategySelector::with_hint(hint)),
            memory_tracker: MemoryTracker::default(),
        }
    }

    /// Create a hash calculator with a custom selector
    pub fn with_selector(selector: StrategySelector) -> Self {
        Self {
            selector: Arc::new(selector),
            memory_tracker: MemoryTracker::default(),
        }
    }

    /// Create a hash calculator with a custom memory limit
    pub fn with_memory_limit(limit: usize) -> Self {
        Self {
            selector: Arc::new(StrategySelector::new()),
            memory_tracker: MemoryTracker::new(limit),
        }
    }

    /// Get the memory tracker for this calculator
    pub fn memory_tracker(&self) -> &MemoryTracker {
        &self.memory_tracker
    }

    /// Calculate hash for byte data
    pub fn calculate_bytes(&self, algorithm: HashAlgorithm, data: &[u8]) -> Result<HashResult> {
        let start_time = Instant::now();

        // Use the registry to get the algorithm implementation
        let algo_impl = algorithm.to_impl();

        // Use the algorithm implementation to hash
        let hash = algo_impl.hash_bytes(data);

        Ok(HashResult {
            algorithm,
            hash,
            input_size: data.len() as u64,
            duration: start_time.elapsed(),
        })
    }

    /// Calculate hash for byte data with specific ED2K variant
    pub fn calculate_bytes_with_variant(
        &self,
        algorithm: HashAlgorithm,
        data: &[u8],
        variant: Ed2kVariant,
    ) -> Result<HashResult> {
        let start_time = Instant::now();

        // Special handling for ED2K with variant
        let hash = if algorithm == HashAlgorithm::ED2K {
            // Create a temporary ED2K algorithm with the specific variant
            use crate::hashing::algorithms::ed2k::Ed2kAlgorithm;
            use crate::hashing::traits::HashAlgorithmImpl;
            let algo = Ed2kAlgorithm::with_variant(variant);
            algo.hash_bytes(data)
        } else {
            // Use the registry to get the algorithm implementation
            let algo_impl = algorithm.to_impl();
            algo_impl.hash_bytes(data)
        };

        Ok(HashResult {
            algorithm,
            hash,
            input_size: data.len() as u64,
            duration: start_time.elapsed(),
        })
    }

    /// Calculate hash for a file using streaming to avoid loading entire file into memory
    pub async fn calculate_file(
        &self,
        file_path: &Path,
        algorithm: HashAlgorithm,
    ) -> Result<HashResult> {
        self.calculate_file_with_config(file_path, algorithm, HashConfig::default())
            .await
    }

    /// Calculate hash for a file with custom configuration
    pub async fn calculate_file_with_config(
        &self,
        file_path: &Path,
        algorithm: HashAlgorithm,
        config: HashConfig,
    ) -> Result<HashResult> {
        // Use the strategy selector to choose the best approach
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: vec![algorithm],
            config,
        };

        let strategy = self.selector.select(&context);
        let result = strategy.execute(context).await?;

        // Extract the single result
        result
            .results
            .into_iter()
            .next()
            .map(|(_, hash_result)| hash_result)
            .ok_or_else(|| {
                Error::Internal(InternalError::hash_calculation(
                    "unknown",
                    "No hash result returned from strategy",
                ))
            })
    }

    /// Calculate hash for a file with progress reporting
    pub async fn calculate_file_with_progress(
        &self,
        file_path: &Path,
        algorithm: HashAlgorithm,
        progress_provider: &dyn ProgressProvider,
    ) -> Result<HashResult> {
        // Use the strategy selector to choose the best approach
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: vec![algorithm],
            config: HashConfig::default(),
        };

        let strategy = self.selector.select(&context);
        let result = strategy
            .execute_with_progress(context, progress_provider)
            .await?;

        // Extract the single result
        result
            .results
            .into_iter()
            .next()
            .map(|(_, hash_result)| hash_result)
            .ok_or_else(|| {
                Error::Internal(InternalError::hash_calculation(
                    "unknown",
                    "No hash result returned from strategy",
                ))
            })
    }

    /// Calculate multiple hashes in a single file pass
    pub async fn calculate_multiple(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        self.calculate_multiple_with_config(file_path, algorithms, HashConfig::default())
            .await
    }

    /// Calculate multiple hashes with custom configuration
    pub async fn calculate_multiple_with_config(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
        config: HashConfig,
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Use the strategy selector to choose the best approach
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config,
        };

        let strategy = self.selector.select(&context);
        let result = strategy.execute(context).await?;

        Ok(result.results)
    }

    /// Calculate multiple hashes in a single file pass with progress reporting
    pub async fn calculate_multiple_with_progress(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
        progress_provider: &dyn ProgressProvider,
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        self.calculate_multiple_with_progress_and_config(
            file_path,
            algorithms,
            progress_provider,
            HashConfig::default(),
        )
        .await
    }

    /// Calculate multiple hashes with progress and custom configuration
    pub async fn calculate_multiple_with_progress_and_config(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
        progress_provider: &dyn ProgressProvider,
        config: HashConfig,
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Use the strategy selector to choose the best approach
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config,
        };

        let strategy = self.selector.select(&context);
        let result = strategy
            .execute_with_progress(context, progress_provider)
            .await?;

        Ok(result.results)
    }

    /// Check if an algorithm is supported
    pub fn supports_algorithm(&self, algorithm: HashAlgorithm) -> bool {
        // Query the registry to check if the algorithm is registered
        AlgorithmRegistry::global()
            .get(&algorithm.to_string())
            .is_some()
    }
}

impl Default for HashCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// Note: ChunkData is now imported from the parallel module

impl HashCalculator {
    /// Calculate multiple hashes in parallel using separate threads
    pub async fn calculate_parallel(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Use strategy with parallel hint
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config: HashConfig::default(),
        };

        // Force parallel strategy by using a custom selector
        let selector = StrategySelector::with_hint(StrategyHint::PreferParallel);
        let strategy = selector.select(&context);
        let result = strategy.execute(context).await?;

        Ok(result.results)
    }

    /// Calculate multiple hashes with true parallel processing using independent queues
    ///
    /// This method implements true parallelism where each algorithm processes different
    /// chunks simultaneously. Fast algorithms can race ahead without waiting for slower ones.
    pub async fn calculate_true_parallel(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Use hybrid strategy which is optimized for true parallel processing
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config: HashConfig::default(),
        };

        // Use hybrid strategy specifically
        let strategy = Arc::new(strategies::HybridStrategy::with_defaults());
        let result = strategy.execute(context).await?;

        Ok(result.results)
    }

    // DEPRECATED: These methods use the old progress system and should be migrated
    // to use ProgressProvider instead of mpsc::Sender<Progress>
    /*
    /// Calculate multiple hashes with true parallel processing and custom configuration
    pub async fn calculate_true_parallel_with_config(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
        config: ParallelConfig,
        progress_tx: mpsc::Sender<Progress>,
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Convert ParallelConfig to HashConfig
        let hash_config = HashConfig {
            chunk_size: config.chunk_size.unwrap_or(9728000),
            parallel_workers: config.queue_depth.unwrap_or(4),
            buffer_size: 64 * 1024,
            use_mmap: false,
            ed2k_variant: Ed2kVariant::Red,
        };

        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config: hash_config,
        };

        // Use parallel strategy with custom config
        let strategy = Arc::new(strategies::ParallelStrategy::new(config));
        let result = strategy.execute(context, Some(progress_tx)).await?;

        Ok(result.results)
    }
    */

    /*
    /// Calculate multiple hashes using the hybrid ring buffer strategy
    #[doc(hidden)]
    pub async fn calculate_hybrid_with_config(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
        config: ParallelConfig,
        progress_tx: mpsc::Sender<Progress>,
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Convert ParallelConfig to HashConfig
        let hash_config = HashConfig {
            chunk_size: config.chunk_size.unwrap_or(9728000),
            parallel_workers: config.queue_depth.unwrap_or(4),
            buffer_size: 64 * 1024,
            use_mmap: false,
            ed2k_variant: Ed2kVariant::Red,
        };

        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config: hash_config,
        };

        // Use hybrid strategy with RING_SIZE and number of algorithms
        let ring_size = 32; // Default ring size
        let worker_count = algorithms.len();
        let strategy = Arc::new(strategies::HybridStrategy::new(ring_size, worker_count));
        let result = strategy.execute(context, Some(progress_tx)).await?;

        Ok(result.results)
    }
    */

    /*
    /// Calculate multiple hashes in parallel with progress reporting
    pub async fn calculate_parallel_with_progress(
        &self,
        file_path: &Path,
        algorithms: &[HashAlgorithm],
        progress_tx: mpsc::Sender<Progress>,
    ) -> Result<HashMap<HashAlgorithm, HashResult>> {
        // Return empty HashMap for empty algorithms
        if algorithms.is_empty() {
            return Ok(HashMap::new());
        }

        // Use strategy with parallel hint and progress
        let context = strategies::HashingContext {
            file_path: PathBuf::from(file_path),
            file_size: if file_path.exists() {
                tokio::fs::metadata(file_path).await?.len()
            } else {
                return Err(Error::Io(IoError::file_not_found(file_path)));
            },
            algorithms: algorithms.to_vec(),
            config: HashConfig::default(),
        };

        // Force parallel strategy
        let selector = StrategySelector::with_hint(StrategyHint::PreferParallel);
        let strategy = selector.select(&context);
        let result = strategy.execute(context, Some(progress_tx)).await?;

        Ok(result.results)
    }
    */
}

// StreamingHasher trait is now defined in traits.rs and re-exported

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::path::Path;
    use std::sync::LazyLock;
    use tempfile::TempDir;
    use tokio::sync::Mutex as AsyncMutex;

    // Mutex to ensure tests run sequentially for memory tracking
    static TEST_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

    /// Test ED2K hash with known test vectors
    #[test]
    fn test_ed2k_known_vectors() {
        let calculator = HashCalculator::new();

        // Test vectors from AniDB specification
        let test_cases: Vec<(&[u8], &str)> = vec![
            (b"", "31d6cfe0d16ae931b73c59d7e0c089c0"),
            (b"a", "bde52cb31de33e46245e05fbdbd6fb24"),
            (b"test content", "a69899814931280e2f527219ad6ac754"),
        ];

        for (input, expected) in test_cases {
            let result = calculator
                .calculate_bytes(HashAlgorithm::ED2K, input)
                .unwrap();
            assert_eq!(
                result.hash, expected,
                "ED2K hash mismatch for input: {input:?}"
            );
            assert_eq!(result.algorithm, HashAlgorithm::ED2K);
            assert_eq!(result.input_size, input.len() as u64);
        }
    }

    /// Test ED2K hash edge case: file size exactly one chunk (9728000 bytes)
    /// This tests the boundary between single-chunk and multi-chunk files
    #[test]
    fn test_ed2k_exact_single_chunk() {
        let calculator = HashCalculator::new();

        // Create data exactly 9728000 bytes (one ED2K chunk)
        let data = vec![0x42u8; 9728000];

        // For a file exactly one chunk size, it should be treated as a single chunk
        // The expected hash is MD4 of the data itself
        let result = calculator
            .calculate_bytes(HashAlgorithm::ED2K, &data)
            .unwrap();

        // This should match the MD4 hash of 9728000 bytes of 0x42
        // We'll verify the behavior is correct
        assert_eq!(result.input_size, 9728000);
        assert_eq!(result.algorithm, HashAlgorithm::ED2K);
    }

    /// Test ED2K hash edge case: file size exactly multiple of chunk size
    /// This is where Blue vs Red variants differ
    #[test]
    fn test_ed2k_exact_multiple_chunks() {
        let calculator = HashCalculator::new();

        // Create data exactly 2 chunks (19456000 bytes)
        let data = vec![0x42u8; 19456000];

        let result = calculator
            .calculate_bytes(HashAlgorithm::ED2K, &data)
            .unwrap();

        assert_eq!(result.input_size, 19456000);
        assert_eq!(result.algorithm, HashAlgorithm::ED2K);

        // For Red variant (AniDB compatible), when file size is exact multiple of chunk size,
        // we should append MD4 hash of empty data to the chunk hashes before final hashing
        // The current implementation uses Blue variant, so this test will help us identify
        // if we need to switch to Red variant
    }

    /// Test ED2K Red variant specifically
    #[test]
    fn test_ed2k_red_variant() {
        use crate::hashing::Ed2kVariant;
        let calculator = HashCalculator::new();

        // Test case where file size is exactly 2 chunks
        let data = vec![0x00u8; 19456000]; // 2 * 9728000

        // Calculate with Red variant (AniDB compatible)
        let result = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &data, Ed2kVariant::Red)
            .unwrap();

        // The Red variant should append MD4("") to chunk hashes when file size
        // is exact multiple of chunk size
        assert_eq!(result.input_size, 19456000);

        // Calculate with Blue variant for comparison
        let blue_result = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &data, Ed2kVariant::Blue)
            .unwrap();

        // Red and Blue should produce different hashes for this edge case
        assert_ne!(result.hash, blue_result.hash);
    }

    /// Test ED2K variants with various file sizes
    #[test]
    fn test_ed2k_variant_behavior() {
        use crate::hashing::Ed2kVariant;
        let calculator = HashCalculator::new();

        // Test 1: File smaller than chunk - both variants should be identical
        let small_data = vec![0x42u8; 1000];
        let red_small = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &small_data, Ed2kVariant::Red)
            .unwrap();
        let blue_small = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &small_data, Ed2kVariant::Blue)
            .unwrap();
        assert_eq!(red_small.hash, blue_small.hash);

        // Test 2: File not multiple of chunk size - both variants should be identical
        let non_multiple = vec![0x42u8; 9728000 + 1000]; // One chunk + 1000 bytes
        let red_non_mult = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &non_multiple, Ed2kVariant::Red)
            .unwrap();
        let blue_non_mult = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &non_multiple, Ed2kVariant::Blue)
            .unwrap();
        assert_eq!(red_non_mult.hash, blue_non_mult.hash);

        // Test 3: File exactly one chunk - both variants should be identical
        let one_chunk = vec![0x42u8; 9728000];
        let red_one = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &one_chunk, Ed2kVariant::Red)
            .unwrap();
        let blue_one = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &one_chunk, Ed2kVariant::Blue)
            .unwrap();
        assert_eq!(red_one.hash, blue_one.hash);

        // Test 4: File exactly multiple chunks - variants should differ
        let exact_multiple = vec![0x42u8; 9728000 * 3]; // Exactly 3 chunks
        let red_exact = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &exact_multiple, Ed2kVariant::Red)
            .unwrap();
        let blue_exact = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &exact_multiple, Ed2kVariant::Blue)
            .unwrap();
        assert_ne!(
            red_exact.hash, blue_exact.hash,
            "Red and Blue variants should differ for exact multiples of chunk size"
        );
    }

    /// Test that default ED2K implementation uses Red variant for AniDB compatibility
    #[test]
    fn test_ed2k_default_is_red_variant() {
        use crate::hashing::Ed2kVariant;
        let calculator = HashCalculator::new();

        // Create file with exactly 2 chunks
        let data = vec![0x00u8; 19456000]; // 2 * 9728000

        // Default calculation
        let default_result = calculator
            .calculate_bytes(HashAlgorithm::ED2K, &data)
            .unwrap();

        // Explicit Red variant
        let red_result = calculator
            .calculate_bytes_with_variant(HashAlgorithm::ED2K, &data, Ed2kVariant::Red)
            .unwrap();

        // They should match - confirming default is Red
        assert_eq!(
            default_result.hash, red_result.hash,
            "Default ED2K should use Red variant for AniDB compatibility"
        );
    }

    /// Test ED2K file streaming with exact chunk boundaries
    #[tokio::test]
    async fn test_ed2k_streaming_chunk_boundaries() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("exact_chunks.bin");

        // Create file with exactly 3 chunks
        let chunk_data = vec![0x55u8; 9728000];
        let mut file_data = Vec::new();
        for _ in 0..3 {
            file_data.extend_from_slice(&chunk_data);
        }
        std::fs::write(&test_file, &file_data).unwrap();

        let calculator = HashCalculator::new();

        // Test with default (should be Red variant for AniDB compatibility)
        let result = calculator
            .calculate_file(&test_file, HashAlgorithm::ED2K)
            .await
            .unwrap();

        assert_eq!(result.input_size, 29184000); // 3 * 9728000
        assert_eq!(result.algorithm, HashAlgorithm::ED2K);

        // Also test in-memory calculation for consistency
        let memory_result = calculator
            .calculate_bytes(HashAlgorithm::ED2K, &file_data)
            .unwrap();

        assert_eq!(result.hash, memory_result.hash);
    }

    /// Test CRC32 hash with known test vectors
    #[test]
    fn test_crc32_known_vectors() {
        let calculator = HashCalculator::new();

        let test_cases: Vec<(&[u8], &str)> = vec![
            (b"", "00000000"),
            (b"a", "e8b7be43"),
            (b"test content", "57f4675d"),
        ];

        for (input, expected) in test_cases {
            let result = calculator
                .calculate_bytes(HashAlgorithm::CRC32, input)
                .unwrap();
            assert_eq!(
                result.hash, expected,
                "CRC32 hash mismatch for input: {input:?}"
            );
            assert_eq!(result.algorithm, HashAlgorithm::CRC32);
        }
    }

    /// Test MD5 hash with known test vectors
    #[test]
    fn test_md5_known_vectors() {
        let calculator = HashCalculator::new();

        let test_cases: Vec<(&[u8], &str)> = vec![
            (b"", "d41d8cd98f00b204e9800998ecf8427e"),
            (b"a", "0cc175b9c0f1b6a831c399e269772661"),
            (b"test content", "9473fdd0d880a43c21b7778d34872157"),
            (
                b"The quick brown fox jumps over the lazy dog",
                "9e107d9d372bb6826bd81d3542a419d6",
            ),
        ];

        for (input, expected) in test_cases {
            let result = calculator
                .calculate_bytes(HashAlgorithm::MD5, input)
                .unwrap();
            assert_eq!(
                result.hash, expected,
                "MD5 hash mismatch for input: {input:?}"
            );
            assert_eq!(result.hash.len(), 32); // MD5 produces 128-bit (32 hex chars) hash
            assert_eq!(result.algorithm, HashAlgorithm::MD5);
        }
    }

    /// Test SHA1 hash with known test vectors
    #[test]
    fn test_sha1_known_vectors() {
        let calculator = HashCalculator::new();

        let test_cases: Vec<(&[u8], &str)> = vec![
            (b"", "da39a3ee5e6b4b0d3255bfef95601890afd80709"),
            (b"a", "86f7e437faa5a7fce15d1ddcb9eaeaea377667b8"),
            (b"test content", "1eebdf4fdc9fc7bf283031b93f9aef3338de9052"),
            (
                b"The quick brown fox jumps over the lazy dog",
                "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12",
            ),
        ];

        for (input, expected) in test_cases {
            let result = calculator
                .calculate_bytes(HashAlgorithm::SHA1, input)
                .unwrap();
            assert_eq!(
                result.hash, expected,
                "SHA1 hash mismatch for input: {input:?}"
            );
            assert_eq!(result.hash.len(), 40); // SHA1 produces 160-bit (40 hex chars) hash
            assert_eq!(result.algorithm, HashAlgorithm::SHA1);
        }
    }

    /// Test TTH (Tiger Tree Hash) with known test vectors
    #[test]
    fn test_tth_known_vectors() {
        let calculator = HashCalculator::new();

        // Test vectors verified against our implementation
        // Note: The empty and single 'a' vectors match DC++ reference implementation
        let test_cases: Vec<(&[u8], &str)> = vec![
            // Empty string/file - matches DC++ reference
            (b"", "lwpnacqdbzryxw3vhjvcj64qbznghohhhzwclnq"),
            // Single character - matches DC++ reference
            (b"a", "czquwh3iyxbf5l3bgyugzhassmxu647ip2ike4y"),
            // Common test strings
            (b"abc", "asd4ujseh5m47pdyb46kbtsqtsgdklbhyxomuia"),
            (b"message digest", "ym432msox5qilih2l4tno62e3o35wygwsbsjoba"),
            (
                b"abcdefghijklmnopqrstuvwxyz",
                "lmhna2vyo465p2rdogtr2cl6xkhzni2x4ccuy5y",
            ),
            // Numbers
            (b"1234567890", "dv7xk7tjhr3jsvavc5t5rszsiswi3e25l2niwuy"),
            // Longer test strings
            (
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "tf74enf7mf2wpde35m23nrsvkjirkyrmtlwahwq",
            ),
        ];

        for (input, expected) in test_cases {
            let result = calculator
                .calculate_bytes(HashAlgorithm::TTH, input)
                .unwrap();
            assert_eq!(
                result.hash, expected,
                "TTH hash mismatch for input: {input:?}"
            );
            assert_eq!(result.algorithm, HashAlgorithm::TTH);
        }
    }

    /// Test TTH with chunk boundary edge cases
    #[test]
    fn test_tth_chunk_boundary_vectors() {
        let calculator = HashCalculator::new();

        // Create test data at chunk boundaries
        let data_1024 = vec![b'x'; 1024]; // Exactly one leaf
        let data_1025 = vec![b'y'; 1025]; // Just over one leaf

        let test_cases: Vec<(&[u8], &str)> = vec![
            // Test string that's exactly one leaf size (1024 bytes)
            (&data_1024, "lhhbwozquw4u4k7wbsuj65kpehbas7bfroyfarq"),
            // Test string slightly over one leaf (1025 bytes)
            (&data_1025, "4o3heknfiuv7vss7dgb234e7ozq477txkktbmjq"),
        ];

        for (input, expected) in test_cases {
            let result = calculator
                .calculate_bytes(HashAlgorithm::TTH, input)
                .unwrap();
            assert_eq!(
                result.hash, expected,
                "TTH hash mismatch for chunk boundary test"
            );
        }
    }

    /// Test TTH chunk boundary handling
    #[test]
    fn test_tth_chunk_boundaries() {
        let calculator = HashCalculator::new();

        // TTH uses 1024-byte leaf size
        const LEAF_SIZE: usize = 1024;

        // Test cases around chunk boundaries
        let test_cases = vec![
            // Just under one leaf
            (LEAF_SIZE - 1, b'a'),
            // Exactly one leaf
            (LEAF_SIZE, b'b'),
            // Just over one leaf
            (LEAF_SIZE + 1, b'c'),
            // Two leaves exactly
            (LEAF_SIZE * 2, b'd'),
            // Multiple leaves
            (LEAF_SIZE * 3, b'e'),
            // Large file (multiple tree levels)
            (LEAF_SIZE * 100, b'f'),
        ];

        for (size, fill_byte) in test_cases {
            let data = vec![fill_byte; size];
            let result = calculator
                .calculate_bytes(HashAlgorithm::TTH, &data)
                .unwrap();

            // Verify we get a valid base32 hash
            assert_eq!(result.hash.len(), 39, "TTH hash should be 39 chars");
            assert!(
                result
                    .hash
                    .chars()
                    .all(|c: char| c.is_ascii_lowercase() || ('2'..='7').contains(&c)),
                "TTH hash should only contain lowercase a-z and 2-7"
            );
        }
    }

    /// Test TTH with various binary patterns
    #[test]
    fn test_tth_binary_patterns() {
        let calculator = HashCalculator::new();

        // Test different binary patterns
        let mut alternating = Vec::new();
        for _ in 0..50 {
            alternating.push(0xAA);
            alternating.push(0x55);
        }

        let test_patterns: Vec<(Vec<u8>, &str)> = vec![
            // All zeros
            (vec![0x00; 100], "expected_hash_for_zeros"),
            // All ones
            (vec![0xFF; 100], "expected_hash_for_ones"),
            // Alternating pattern
            (alternating, "expected_hash_for_alternating"),
            // Sequential bytes
            ((0..=255u8).collect(), "expected_hash_for_sequential"),
        ];

        for (data, _description) in test_patterns {
            let result = calculator
                .calculate_bytes(HashAlgorithm::TTH, &data)
                .unwrap();

            // Just verify format since we don't have the exact expected values
            assert_eq!(result.hash.len(), 39);
            assert!(
                result
                    .hash
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c))
            );
        }
    }

    /// Test TTH tree structure properties
    #[test]
    fn test_tth_tree_properties() {
        let calculator = HashCalculator::new();

        // When data changes, hash should change
        let data1 = vec![1u8; 2048];
        let data2 = vec![2u8; 2048];

        let hash1 = calculator
            .calculate_bytes(HashAlgorithm::TTH, &data1)
            .unwrap()
            .hash;
        let hash2 = calculator
            .calculate_bytes(HashAlgorithm::TTH, &data2)
            .unwrap()
            .hash;

        assert_ne!(
            hash1, hash2,
            "Different data should produce different hashes"
        );

        // Same data should produce same hash
        let hash1_repeat = calculator
            .calculate_bytes(HashAlgorithm::TTH, &data1)
            .unwrap()
            .hash;

        assert_eq!(hash1, hash1_repeat, "Same data should produce same hash");
    }

    /// Test file hashing with temporary files
    #[tokio::test]
    async fn test_calculate_file_hash() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.mkv");
        std::fs::write(&test_file, b"test file content").unwrap();

        let calculator = HashCalculator::new();

        // Test each algorithm
        for algorithm in [
            HashAlgorithm::ED2K,
            HashAlgorithm::CRC32,
            HashAlgorithm::MD5,
            HashAlgorithm::SHA1,
            HashAlgorithm::TTH,
        ] {
            let result = calculator
                .calculate_file(&test_file, algorithm)
                .await
                .unwrap();

            assert_eq!(result.algorithm, algorithm);
            assert_eq!(result.input_size, 17); // "test file content".len()
            assert!(!result.hash.is_empty());
            assert!(result.duration.as_nanos() > 0);
        }
    }

    /// Test large file hashing with streaming
    #[tokio::test]
    async fn test_large_file_streaming() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let temp_dir = TempDir::new().unwrap();
        let large_file = temp_dir.path().join("large.mkv");

        // Create a 1MB file
        let content = vec![42u8; 1024 * 1024];
        std::fs::write(&large_file, content).unwrap();

        let calculator = HashCalculator::new();
        let result = calculator
            .calculate_file(&large_file, HashAlgorithm::ED2K)
            .await
            .unwrap();

        assert_eq!(result.input_size, 1024 * 1024);
        assert!(!result.hash.is_empty());
        // Should complete reasonably quickly
        assert!(result.duration.as_secs() < 5);
    }

    /// Test progress reporting during hash calculation
    #[tokio::test]
    async fn test_hash_progress_reporting() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("progress_test.mkv");

        // Create a moderately sized file (100KB)
        let content = vec![0u8; 100 * 1024];
        std::fs::write(&test_file, content).unwrap();

        let calculator = HashCalculator::new();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<crate::Progress>(10);

        // Start hash calculation with progress
        let provider = crate::progress::ChannelAdapter::new(progress_tx);
        let hash_task = tokio::spawn(async move {
            calculator
                .calculate_file_with_progress(&test_file, HashAlgorithm::ED2K, &provider)
                .await
        });

        // Collect progress updates
        let mut progress_updates = Vec::new();
        while let Some(progress) = progress_rx.recv().await {
            progress_updates.push(progress.clone());
            if progress.percentage >= 100.0 {
                break;
            }
        }

        // Verify hash result
        let result = hash_task.await.unwrap().unwrap();
        assert_eq!(result.algorithm, HashAlgorithm::ED2K);

        // Verify progress updates
        assert!(!progress_updates.is_empty());
        assert!(progress_updates.iter().any(|p| p.percentage == 100.0));
        assert!(progress_updates.iter().any(|p| p.bytes_processed > 0));
    }

    /// Test concurrent hash calculation
    #[tokio::test]
    async fn test_concurrent_hash_calculation() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("concurrent_test.mkv");
        std::fs::write(&test_file, b"concurrent test content").unwrap();

        let calculator = HashCalculator::new();

        // Calculate multiple hashes concurrently
        let algorithms = vec![
            HashAlgorithm::ED2K,
            HashAlgorithm::CRC32,
            HashAlgorithm::MD5,
            HashAlgorithm::SHA1,
            HashAlgorithm::TTH,
        ];
        let results = calculator
            .calculate_multiple(&test_file, &algorithms)
            .await
            .unwrap();

        assert_eq!(results.len(), 5);
        for (algorithm, result) in results {
            assert_eq!(result.algorithm, algorithm);
            assert!(!result.hash.is_empty());
            assert_eq!(result.input_size, 23); // "concurrent test content".len()
        }
    }

    /// Test empty file handling
    #[tokio::test]
    async fn test_empty_file_handling() {
        let _guard = TEST_MUTEX.lock().await;
        crate::buffer::reset_memory_tracking();

        let temp_dir = TempDir::new().unwrap();
        let empty_file = temp_dir.path().join("empty.mkv");
        std::fs::write(&empty_file, b"").unwrap();

        let calculator = HashCalculator::new();

        for algorithm in [
            HashAlgorithm::ED2K,
            HashAlgorithm::CRC32,
            HashAlgorithm::MD5,
            HashAlgorithm::SHA1,
            HashAlgorithm::TTH,
        ] {
            let result = calculator
                .calculate_file(&empty_file, algorithm)
                .await
                .unwrap();

            assert_eq!(result.input_size, 0);
            assert!(!result.hash.is_empty()); // Even empty files should have a hash
            assert_eq!(result.algorithm, algorithm);
        }
    }

    /// Test error handling for non-existent files
    #[tokio::test]
    async fn test_nonexistent_file_error() {
        let calculator = HashCalculator::new();
        let non_existent = Path::new("/non/existent/file.mkv");

        let result = calculator
            .calculate_file(non_existent, HashAlgorithm::ED2K)
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Io(ref io_err) if io_err.kind == crate::error::IoErrorKind::FileNotFound => {
                assert_eq!(io_err.path, Some(non_existent.to_path_buf()));
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }

    /// Test hash algorithm validation
    #[test]
    fn test_hash_algorithm_validation() {
        let calculator = HashCalculator::new();

        // All algorithms should be supported
        assert!(calculator.supports_algorithm(HashAlgorithm::ED2K));
        assert!(calculator.supports_algorithm(HashAlgorithm::CRC32));
        assert!(calculator.supports_algorithm(HashAlgorithm::MD5));
        assert!(calculator.supports_algorithm(HashAlgorithm::SHA1));
        assert!(calculator.supports_algorithm(HashAlgorithm::TTH));
    }

    /// Test hash result serialization
    #[test]
    fn test_hash_result_serialization() {
        let result = HashResult {
            algorithm: HashAlgorithm::ED2K,
            hash: "deadbeef".to_string(),
            input_size: 1024,
            duration: Duration::from_millis(100),
        };

        // Should be able to serialize to JSON
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("ED2K"));
        assert!(json.contains("deadbeef"));
        assert!(json.contains("1024"));

        // Should be able to deserialize back
        let deserialized: HashResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.algorithm, HashAlgorithm::ED2K);
        assert_eq!(deserialized.hash, "deadbeef");
        assert_eq!(deserialized.input_size, 1024);
    }

    proptest! {
        #[test]
        fn test_hash_consistency(data: Vec<u8>) {
            let calculator = HashCalculator::new();

            // Same input should always produce same hash
            let result1 = calculator.calculate_bytes(HashAlgorithm::ED2K, &data).unwrap();
            let result2 = calculator.calculate_bytes(HashAlgorithm::ED2K, &data).unwrap();

            prop_assert_eq!(result1.hash, result2.hash);
            prop_assert_eq!(result1.input_size, data.len() as u64);
        }
    }

    proptest! {
        #[test]
        fn test_hash_determinism(data: Vec<u8>) {
            let calculator = HashCalculator::new();

            for algorithm in [
                HashAlgorithm::ED2K,
                HashAlgorithm::CRC32,
                    HashAlgorithm::MD5,
                HashAlgorithm::SHA1,
                HashAlgorithm::TTH,
            ] {
                let result = calculator.calculate_bytes(algorithm, &data).unwrap();

                prop_assert_eq!(result.algorithm, algorithm);
                prop_assert_eq!(result.input_size, data.len() as u64);
                prop_assert!(!result.hash.is_empty());
                // TTH uses base32, others use hex
                if algorithm == HashAlgorithm::TTH {
                    prop_assert!(result.hash.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
                } else {
                    prop_assert!(result.hash.chars().all(|c| c.is_ascii_hexdigit()));
                }
            }
        }
    }

    /// Memory usage test for large inputs
    #[test]
    fn test_memory_usage_large_input() {
        let calculator = HashCalculator::new();

        // Test with progressively larger inputs
        for size in [1024, 10_240, 102_400] {
            let data = vec![0u8; size];
            let result = calculator
                .calculate_bytes(HashAlgorithm::ED2K, &data)
                .unwrap();

            assert_eq!(result.input_size, size as u64);
            assert!(!result.hash.is_empty());
        }

        // Memory usage should remain constant regardless of input size
        // (This is more of a benchmark/profiling test in reality)
    }

    /// Test hash algorithm enum properties
    #[test]
    fn test_hash_algorithm_enum_properties() {
        // Test Debug formatting
        assert_eq!(format!("{:?}", HashAlgorithm::ED2K), "ED2K");
        assert_eq!(format!("{:?}", HashAlgorithm::CRC32), "CRC32");
        assert_eq!(format!("{:?}", HashAlgorithm::MD5), "MD5");
        assert_eq!(format!("{:?}", HashAlgorithm::SHA1), "SHA1");
        assert_eq!(format!("{:?}", HashAlgorithm::TTH), "TTH");

        // Test equality
        assert_eq!(HashAlgorithm::ED2K, HashAlgorithm::ED2K);
        assert_ne!(HashAlgorithm::ED2K, HashAlgorithm::CRC32);

        // Test clone
        let algorithm = HashAlgorithm::ED2K;
        let cloned = algorithm;
        assert_eq!(algorithm, cloned);

        // Test as hash map key
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(HashAlgorithm::ED2K, "ed2k_hash");
        assert_eq!(map.get(&HashAlgorithm::ED2K), Some(&"ed2k_hash"));
    }
}
