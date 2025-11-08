//! MD5 hash algorithm implementation

use crate::hashing::traits::{HashAlgorithmImpl, StreamingHasher};
use md5::{Digest as Md5Digest, Md5};

pub struct Md5Algorithm;

/// MD5 streaming hasher
struct Md5StreamingHasher {
    hasher: Md5,
}

impl Md5StreamingHasher {
    fn new() -> Self {
        Self { hasher: Md5::new() }
    }
}

impl StreamingHasher for Md5StreamingHasher {
    fn update(&mut self, data: &[u8]) {
        Md5Digest::update(&mut self.hasher, data);
    }

    fn finalize(self: Box<Self>) -> String {
        format!("{:x}", Md5Digest::finalize(self.hasher))
    }
}

impl HashAlgorithmImpl for Md5Algorithm {
    fn id(&self) -> &'static str {
        "md5"
    }

    fn display_name(&self) -> &'static str {
        "MD5"
    }

    fn create_hasher(&self) -> Box<dyn StreamingHasher> {
        Box::new(Md5StreamingHasher::new())
    }

    fn hash_bytes(&self, data: &[u8]) -> String {
        // Use the streaming hasher for consistency
        let mut hasher = Md5StreamingHasher::new();
        hasher.update(data);
        Box::new(hasher).finalize()
    }

    fn memory_overhead(&self) -> usize {
        1024 // Minimal state
    }
}
