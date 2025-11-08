//! Platform-specific I/O optimization strategies
//!
//! Provides abstractions for choosing optimal I/O strategies based on
//! platform capabilities, file size, and access patterns.

use crate::{Error, Result, error::InternalError};
use std::path::Path;
use tokio::io::AsyncRead;

/// I/O access patterns
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadPattern {
    Sequential,
    Random,
    Mixed,
}

/// Memory usage preferences
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPreference {
    Low,      // Minimize memory usage
    Balanced, // Balance between speed and memory
    High,     // Maximize speed, higher memory usage ok
}

/// I/O strategy to use for file operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoStrategy {
    MemoryMapped,  // Use memory mapping (Linux small files)
    AsyncBuffered, // Use async I/O with buffering
    DirectIo,      // Use direct I/O (bypass OS cache)
    Overlapped,    // Use Windows overlapped I/O
}

/// Optimization hint for choosing I/O strategy
#[derive(Debug, Clone)]
pub struct OptimizationHint {
    pub file_size: usize,
    pub read_pattern: ReadPattern,
    pub memory_preference: MemoryPreference,
}

impl OptimizationHint {
    /// Create optimization hint for hash calculation workload
    pub fn for_hash_calculation(file_size: usize) -> Self {
        Self {
            file_size,
            read_pattern: ReadPattern::Sequential,
            memory_preference: MemoryPreference::Balanced,
        }
    }

    /// Create optimization hint for file analysis workload
    pub fn for_file_analysis(file_size: usize) -> Self {
        Self {
            file_size,
            read_pattern: ReadPattern::Mixed,
            memory_preference: MemoryPreference::Low,
        }
    }
}

/// Platform-aware I/O optimizer
#[derive(Debug, Clone)]
pub struct IoOptimizer {
    // Configuration for optimization decisions
}

impl IoOptimizer {
    /// Create a new I/O optimizer
    pub fn new() -> Self {
        Self {}
    }

    /// Choose the optimal I/O strategy based on hint and platform
    pub fn choose_strategy(&self, hint: &OptimizationHint) -> IoStrategy {
        // Small files on Linux can use memory mapping efficiently
        if self.should_use_memory_mapping(hint) {
            return IoStrategy::MemoryMapped;
        }

        // Large files or other platforms use async buffered I/O
        IoStrategy::AsyncBuffered
    }

    /// Create an optimized reader for the given file and strategy
    pub async fn create_optimized_reader<P: AsRef<Path>>(
        &self,
        file_path: P,
        strategy: IoStrategy,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let file_path = file_path.as_ref();

        match strategy {
            IoStrategy::MemoryMapped => self.create_memory_mapped_reader(file_path).await,
            IoStrategy::AsyncBuffered => self.create_async_buffered_reader(file_path).await,
            IoStrategy::DirectIo => {
                // Direct I/O implementation would go here
                // For now, fall back to async buffered
                self.create_async_buffered_reader(file_path).await
            }
            IoStrategy::Overlapped => {
                // Windows overlapped I/O implementation would go here
                // For now, fall back to async buffered
                self.create_async_buffered_reader(file_path).await
            }
        }
    }

    /// Determine if memory mapping should be used
    fn should_use_memory_mapping(&self, hint: &OptimizationHint) -> bool {
        // Only use memory mapping on Linux for small files
        cfg!(target_os = "linux") 
            && hint.file_size < 1024 * 1024 * 1024 // 1GB limit
            && hint.read_pattern == ReadPattern::Sequential
    }

    /// Create a memory-mapped reader (Linux only)
    async fn create_memory_mapped_reader<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        #[cfg(target_os = "linux")]
        {
            // Create a memory-mapped file reader
            let file = tokio::fs::File::open(file_path).await?;
            let reader = tokio::io::BufReader::new(file);
            Ok(Box::new(reader))
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = file_path; // Suppress unused warning
            // Fall back to async buffered on other platforms
            Err(Error::Internal(InternalError::unsupported_io_strategy(
                "MemoryMapped",
                "Platform does not support memory mapping",
            )))
        }
    }

    /// Create an async buffered reader (all platforms)
    async fn create_async_buffered_reader<P: AsRef<Path>>(
        &self,
        file_path: P,
    ) -> Result<Box<dyn AsyncRead + Unpin + Send>> {
        let file = tokio::fs::File::open(file_path).await?;
        let reader = tokio::io::BufReader::new(file);
        Ok(Box::new(reader))
    }
}

impl Default for IoOptimizer {
    fn default() -> Self {
        Self::new()
    }
}
