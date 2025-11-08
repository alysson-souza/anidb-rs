//! ED2K hash algorithm implementation

use crate::hashing::Ed2kVariant;
use crate::hashing::traits::{HashAlgorithmImpl, StreamingHasher};
use md4::{Digest, Md4};

pub struct Ed2kAlgorithm {
    variant: Ed2kVariant,
}

impl Ed2kAlgorithm {
    /// Create a new ED2K algorithm with default Red variant
    pub fn new() -> Self {
        Self {
            variant: Ed2kVariant::Red,
        }
    }

    /// Create a new ED2K algorithm with specific variant
    pub fn with_variant(variant: Ed2kVariant) -> Self {
        Self { variant }
    }
}

impl Default for Ed2kAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

/// ED2K streaming hasher with internal accumulator for decoupled I/O
pub(crate) struct Ed2kStreamingHasher {
    // Internal accumulator for building 9.5MB chunks
    accumulator: Vec<u8>,
    accumulator_size: usize,

    // Completed chunk hashes
    chunk_hashes: Vec<u8>,

    // Total bytes processed
    bytes_processed: usize,

    // ED2K variant
    variant: Ed2kVariant,
}

impl Ed2kStreamingHasher {
    const CHUNK_SIZE: usize = 9_728_000; // 9.5MB ED2K chunk size

    fn new(variant: Ed2kVariant) -> Self {
        Self {
            accumulator: Vec::with_capacity(Self::CHUNK_SIZE),
            accumulator_size: 0,
            chunk_hashes: Vec::new(),
            bytes_processed: 0,
            variant,
        }
    }

    fn process_chunk(&mut self) {
        let mut hasher = Md4::new();
        hasher.update(&self.accumulator[..self.accumulator_size]);
        self.chunk_hashes.extend_from_slice(&hasher.finalize());

        // Reset accumulator
        self.accumulator.clear();
        self.accumulator_size = 0;
    }
}

impl StreamingHasher for Ed2kStreamingHasher {
    fn update(&mut self, data: &[u8]) {
        let mut remaining = data;

        while !remaining.is_empty() {
            let space_in_accumulator = Self::CHUNK_SIZE - self.accumulator_size;
            let to_copy = remaining.len().min(space_in_accumulator);

            // Accumulate data
            self.accumulator.extend_from_slice(&remaining[..to_copy]);
            self.accumulator_size += to_copy;
            remaining = &remaining[to_copy..];

            // Process complete chunk
            if self.accumulator_size == Self::CHUNK_SIZE {
                self.process_chunk();
            }
        }

        self.bytes_processed += data.len();
    }

    fn finalize(mut self: Box<Self>) -> String {
        if self.bytes_processed == 0 {
            // Empty file
            return format!("{:x}", Md4::new().finalize());
        }

        if self.bytes_processed < Self::CHUNK_SIZE {
            // File smaller than one chunk (not equal)
            let mut hasher = Md4::new();
            hasher.update(&self.accumulator[..self.accumulator_size]);
            return format!("{:x}", hasher.finalize());
        }

        // Process any remaining data (including exactly one chunk)
        if self.accumulator_size > 0 {
            self.process_chunk();
        }

        // Special case: exactly one chunk
        if self.bytes_processed == Self::CHUNK_SIZE
            && (self.variant == Ed2kVariant::Blue || self.chunk_hashes.len() == 16)
        {
            // Blue variant or only one chunk hash: return the chunk hash directly
            let mut result = String::new();
            for byte in &self.chunk_hashes[..16] {
                result.push_str(&format!("{byte:02x}"));
            }
            return result;
        }
        // Red variant falls through to handle multiple chunks with empty hash

        // Red variant: If file size is exact multiple of chunk size,
        // append MD4 hash of empty data
        if self.variant == Ed2kVariant::Red && self.bytes_processed.is_multiple_of(Self::CHUNK_SIZE)
        {
            let mut empty_hasher = Md4::new();
            empty_hasher.update(b"");
            self.chunk_hashes
                .extend_from_slice(&empty_hasher.finalize());
        }

        // Hash all chunk hashes
        let mut final_hasher = Md4::new();
        final_hasher.update(&self.chunk_hashes);
        format!("{:x}", final_hasher.finalize())
    }
}

impl HashAlgorithmImpl for Ed2kAlgorithm {
    fn id(&self) -> &'static str {
        "ed2k"
    }

    fn display_name(&self) -> &'static str {
        "ED2K"
    }

    fn create_hasher(&self) -> Box<dyn StreamingHasher> {
        // Use the variant from the algorithm instance
        Box::new(Ed2kStreamingHasher::new(self.variant))
    }

    fn hash_bytes(&self, data: &[u8]) -> String {
        // Use the streaming hasher for consistency
        let mut hasher = Ed2kStreamingHasher::new(self.variant);
        hasher.update(data);
        Box::new(hasher).finalize()
    }

    fn memory_overhead(&self) -> usize {
        1024 // Minimal state, accumulator allocated separately
    }

    fn has_variants(&self) -> bool {
        true
    }

    fn variants(&self) -> Vec<&'static str> {
        vec!["red", "blue"]
    }
}
