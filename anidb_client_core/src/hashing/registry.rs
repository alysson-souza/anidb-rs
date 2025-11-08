//! Central registry for hash algorithm implementations

use super::traits::HashAlgorithmImpl;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Central registry for all hash algorithms
pub struct AlgorithmRegistry {
    algorithms: RwLock<HashMap<String, Arc<dyn HashAlgorithmImpl>>>,
}

impl AlgorithmRegistry {
    /// Create a new empty registry
    fn new() -> Self {
        Self {
            algorithms: RwLock::new(HashMap::new()),
        }
    }

    /// Get the global registry instance
    pub fn global() -> &'static Self {
        static INSTANCE: OnceCell<AlgorithmRegistry> = OnceCell::new();
        INSTANCE.get_or_init(|| {
            let mut registry = Self::new();
            // Register all built-in algorithms
            super::algorithms::register_all(&mut registry);
            registry
        })
    }

    /// Register a new algorithm
    pub fn register(&mut self, algorithm: impl HashAlgorithmImpl + 'static) {
        let mut algorithms = self.algorithms.write().unwrap();
        let id = algorithm.id().to_string();
        algorithms.insert(id, Arc::new(algorithm));
    }

    /// Get algorithm by ID
    pub fn get(&self, id: &str) -> Option<Arc<dyn HashAlgorithmImpl>> {
        let algorithms = self.algorithms.read().unwrap();
        algorithms.get(id).cloned()
    }

    /// List all registered algorithms
    pub fn list(&self) -> Vec<&'static str> {
        let algorithms = self.algorithms.read().unwrap();
        let mut ids: Vec<_> = algorithms.keys().map(|k| k.as_str()).collect();
        ids.sort();

        // Return static string slices that match our algorithm IDs
        ids.into_iter()
            .filter_map(|id| match id {
                "ed2k" => Some("ed2k"),
                "crc32" => Some("crc32"),
                "blake3" => Some("blake3"),
                "md5" => Some("md5"),
                "sha1" => Some("sha1"),
                "tth" => Some("tth"),
                _ => None,
            })
            .collect()
    }
}
