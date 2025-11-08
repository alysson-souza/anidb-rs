//! SHA1 hash algorithm implementation

use crate::hashing::traits::{HashAlgorithmImpl, StreamingHasher};
use sha1::{Digest as Sha1Digest, Sha1};

pub struct Sha1Algorithm;

/// SHA1 streaming hasher
struct Sha1StreamingHasher {
    hasher: Sha1,
}

impl Sha1StreamingHasher {
    fn new() -> Self {
        Self {
            hasher: Sha1::new(),
        }
    }
}

impl StreamingHasher for Sha1StreamingHasher {
    fn update(&mut self, data: &[u8]) {
        Sha1Digest::update(&mut self.hasher, data);
    }

    fn finalize(self: Box<Self>) -> String {
        format!("{:x}", Sha1Digest::finalize(self.hasher))
    }
}

impl HashAlgorithmImpl for Sha1Algorithm {
    fn id(&self) -> &'static str {
        "sha1"
    }

    fn display_name(&self) -> &'static str {
        "SHA1"
    }

    fn create_hasher(&self) -> Box<dyn StreamingHasher> {
        Box::new(Sha1StreamingHasher::new())
    }

    fn hash_bytes(&self, data: &[u8]) -> String {
        // Use the streaming hasher for consistency
        let mut hasher = Sha1StreamingHasher::new();
        hasher.update(data);
        Box::new(hasher).finalize()
    }

    fn memory_overhead(&self) -> usize {
        1024 // Minimal state
    }
}
