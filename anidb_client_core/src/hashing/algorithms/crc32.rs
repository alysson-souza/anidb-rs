//! CRC32 hash algorithm implementation

use crate::hashing::traits::{HashAlgorithmImpl, StreamingHasher};
use crc32fast::Hasher as Crc32Hasher;

pub struct Crc32Algorithm;

/// CRC32 streaming hasher
struct Crc32StreamingHasher {
    hasher: Crc32Hasher,
}

impl Crc32StreamingHasher {
    fn new() -> Self {
        Self {
            hasher: Crc32Hasher::new(),
        }
    }
}

impl StreamingHasher for Crc32StreamingHasher {
    fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    fn finalize(self: Box<Self>) -> String {
        format!("{:08x}", self.hasher.finalize())
    }
}

impl HashAlgorithmImpl for Crc32Algorithm {
    fn id(&self) -> &'static str {
        "crc32"
    }

    fn display_name(&self) -> &'static str {
        "CRC32"
    }

    fn create_hasher(&self) -> Box<dyn StreamingHasher> {
        Box::new(Crc32StreamingHasher::new())
    }

    fn hash_bytes(&self, data: &[u8]) -> String {
        // Use the streaming hasher for consistency
        let mut hasher = Crc32StreamingHasher::new();
        hasher.update(data);
        Box::new(hasher).finalize()
    }

    fn memory_overhead(&self) -> usize {
        1024 // Minimal state
    }
}
