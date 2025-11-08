//! AniDB Client Core Library
//!
//! This is the core library for the AniDB client, providing file processing,
//! hashing, caching, and FFI capabilities.

pub mod api;
pub mod batch_processor;
pub mod buffer;
#[cfg(feature = "database")]
pub mod database;
pub mod error;
pub mod ffi;
pub mod ffi_inline;
pub mod ffi_memory;
pub mod ffi_optimization;
pub mod file_io;
pub mod file_processing;
pub mod hashing;
pub mod identification;
pub mod memory;
pub mod pipeline;
pub mod platform;
pub mod progress;
pub mod protocol;
pub mod security;

// Test utilities module (available for tests)
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

// Mock implementations and testing utilities have been moved to anidb-test-utils crate

// Re-export main types
pub use api::{
    AniDBClient, AnimeIdentification, BatchOptions, BatchResult, FileResult, IdentificationSource,
    ProcessOptions,
};
pub use buffer::{
    DEFAULT_BUFFER_SIZE, DEFAULT_MEMORY_LIMIT, allocate_buffer, get_memory_limit, memory_used,
    release_buffer, set_memory_limit,
};
#[cfg(feature = "database")]
pub use database::{Database, DatabaseStats};
pub use error::{Error, Result};
pub use file_io::{FileProcessingResult, FileProcessor, ProcessingStatus};
pub use hashing::{Ed2kVariant, HashAlgorithm, HashCalculator, HashResult, ParallelConfig};
pub use progress::{
    ChannelAdapter, NullProvider, ProgressProvider, ProgressUpdate, SharedProvider,
};

/// Progress information for file and hash processing operations
#[derive(Debug, Clone)]
pub struct Progress {
    pub percentage: f64,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub throughput_mbps: f64,
    pub current_operation: String,
    pub memory_usage_bytes: Option<u64>,
    pub peak_memory_bytes: Option<u64>,
    pub buffer_size: Option<usize>,
}

/// Core client configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClientConfig {
    pub max_concurrent_files: usize,
    pub chunk_size: usize,
    pub max_memory_usage: usize,
    pub username: Option<String>,
    pub password: Option<String>,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            max_concurrent_files: 4,
            chunk_size: 64 * 1024,               // 64KB base chunk size
            max_memory_usage: 500 * 1024 * 1024, // 500MB default
            username: None,
            password: None,
            client_name: None,
            client_version: None,
        }
    }
}

impl ClientConfig {
    /// Create a test configuration
    pub fn test() -> Self {
        Self {
            max_concurrent_files: 2,
            chunk_size: 1024,                    // 1KB chunks for faster tests
            max_memory_usage: 100 * 1024 * 1024, // 100MB for tests
            username: Some("testuser".to_string()),
            password: Some("testpass".to_string()),
            client_name: Some("testclient".to_string()),
            client_version: Some("1".to_string()),
        }
    }
}
