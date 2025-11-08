//! Tiger Tree Hash (TTH) algorithm implementation

use crate::hashing::traits::{HashAlgorithmImpl, StreamingHasher};
use tiger::{Digest as TigerDigest, Tiger};

pub struct TthAlgorithm;

/// TTH streaming hasher with proper memory-efficient streaming
struct TthStreamingHasher {
    // Buffer for accumulating data until we have a complete leaf
    leaf_buffer: Vec<u8>,
    // Computed leaf hashes (24 bytes each for Tiger-192)
    leaf_hashes: Vec<u8>,
    // Total bytes processed
    total_bytes: usize,
}

impl TthStreamingHasher {
    const LEAF_SIZE: usize = 1024;
    const HASH_SIZE: usize = 24; // Tiger-192 produces 192-bit (24-byte) hashes

    fn new() -> Self {
        Self {
            leaf_buffer: Vec::with_capacity(Self::LEAF_SIZE),
            leaf_hashes: Vec::new(),
            total_bytes: 0,
        }
    }

    fn process_leaf(&mut self) {
        if self.leaf_buffer.is_empty() {
            return;
        }

        let mut hasher = Tiger::new();
        TigerDigest::update(&mut hasher, [0x00]); // Leaf prefix
        TigerDigest::update(&mut hasher, &self.leaf_buffer);
        let hash = TigerDigest::finalize(hasher);
        self.leaf_hashes.extend_from_slice(&hash);

        self.leaf_buffer.clear();
    }
}

impl StreamingHasher for TthStreamingHasher {
    fn update(&mut self, data: &[u8]) {
        let mut remaining = data;

        while !remaining.is_empty() {
            let space_in_buffer = Self::LEAF_SIZE - self.leaf_buffer.len();
            let to_copy = remaining.len().min(space_in_buffer);

            self.leaf_buffer.extend_from_slice(&remaining[..to_copy]);
            remaining = &remaining[to_copy..];

            if self.leaf_buffer.len() == Self::LEAF_SIZE {
                self.process_leaf();
            }
        }

        self.total_bytes += data.len();
    }

    fn finalize(mut self: Box<Self>) -> String {
        if self.total_bytes == 0 {
            // Empty file TTH
            return "lwpnacqdbzryxw3vhjvcj64qbznghohhhzwclnq".to_string();
        }

        // Process any remaining data in the buffer
        if !self.leaf_buffer.is_empty() {
            self.process_leaf();
        }

        if self.leaf_hashes.len() == Self::HASH_SIZE {
            // Single leaf - convert to base32
            base32_encode(&self.leaf_hashes)
        } else {
            // Multiple leaves - build Merkle tree
            let root_hash = build_merkle_tree(&self.leaf_hashes);
            base32_encode(&root_hash)
        }
    }
}

fn build_merkle_tree(leaf_hashes: &[u8]) -> Vec<u8> {
    if leaf_hashes.len() <= 24 {
        // Single hash
        return leaf_hashes.to_vec();
    }

    // Convert to owned vectors for the tree building process
    let mut current_level: Vec<Vec<u8>> =
        leaf_hashes.chunks(24).map(|chunk| chunk.to_vec()).collect();

    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        let mut i = 0;

        while i < current_level.len() {
            if i + 1 < current_level.len() {
                // We have a pair - hash them together
                let mut hasher = Tiger::new();
                TigerDigest::update(&mut hasher, [0x01]); // Internal node prefix
                TigerDigest::update(&mut hasher, &current_level[i]);
                TigerDigest::update(&mut hasher, &current_level[i + 1]);
                let hash = TigerDigest::finalize(hasher);
                next_level.push(hash.to_vec());
                i += 2;
            } else {
                // Odd node - promote to next level unchanged
                next_level.push(current_level[i].clone());
                i += 1;
            }
        }

        current_level = next_level;
    }

    current_level.into_iter().next().unwrap_or_default()
}

fn base32_encode(data: &[u8]) -> String {
    const BASE32_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut result = String::new();

    for chunk in data.chunks(5) {
        let mut bits = 0u64;
        for (i, &byte) in chunk.iter().enumerate() {
            bits |= (byte as u64) << (32 - (i * 8));
        }

        for i in 0..8 {
            if i * 5 < chunk.len() * 8 {
                let index = ((bits >> (35 - i * 5)) & 0x1F) as usize;
                result.push(BASE32_ALPHABET[index] as char);
            }
        }
    }

    // Trim padding
    result.trim_end_matches('=').to_lowercase()
}

impl HashAlgorithmImpl for TthAlgorithm {
    fn id(&self) -> &'static str {
        "tth"
    }

    fn display_name(&self) -> &'static str {
        "TTH"
    }

    fn create_hasher(&self) -> Box<dyn StreamingHasher> {
        Box::new(TthStreamingHasher::new())
    }

    fn hash_bytes(&self, data: &[u8]) -> String {
        // Use the streaming hasher for consistency
        let mut hasher = TthStreamingHasher::new();
        hasher.update(data);
        Box::new(hasher).finalize()
    }

    fn memory_overhead(&self) -> usize {
        1024 * 256 // TTH tree storage estimate
    }
}
