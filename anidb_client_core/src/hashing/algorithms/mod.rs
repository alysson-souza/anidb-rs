//! Hash algorithm implementations

use super::registry::AlgorithmRegistry;

mod crc32;
pub mod ed2k;
mod md5;
mod sha1;
mod tth;

/// Register all built-in algorithms with the registry
pub(crate) fn register_all(registry: &mut AlgorithmRegistry) {
    registry.register(ed2k::Ed2kAlgorithm::new());
    registry.register(crc32::Crc32Algorithm);
    registry.register(md5::Md5Algorithm);
    registry.register(sha1::Sha1Algorithm);
    registry.register(tth::TthAlgorithm);
}
