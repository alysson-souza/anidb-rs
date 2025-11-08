//! Core traits for the hash algorithm extensibility system

use std::sync::Arc;

/// Core trait that all hash algorithms must implement
pub trait HashAlgorithmImpl: Send + Sync {
    /// Unique identifier for this algorithm
    fn id(&self) -> &'static str;

    /// Display name for user interfaces
    fn display_name(&self) -> &'static str;

    /// Create a new streaming hasher instance
    fn create_hasher(&self) -> Box<dyn StreamingHasher>;

    /// Calculate hash for in-memory data
    fn hash_bytes(&self, data: &[u8]) -> String;

    /// Estimated memory usage for hasher state
    fn memory_overhead(&self) -> usize;

    /// Whether this algorithm has variants (like ED2K red/blue)
    fn has_variants(&self) -> bool {
        false
    }

    /// Get available variants if any
    fn variants(&self) -> Vec<&'static str> {
        vec![]
    }
}

/// Trait for streaming hash calculation
pub trait StreamingHasher: Send {
    /// Update the hasher with new data
    fn update(&mut self, data: &[u8]);

    /// Finalize the hash calculation and return the result
    fn finalize(self: Box<Self>) -> String;
}

/// Extension trait for HashAlgorithm enum to provide adapter to new system
pub trait HashAlgorithmExt {
    /// Convert enum to trait implementation
    fn to_impl(&self) -> Arc<dyn HashAlgorithmImpl>;
}
