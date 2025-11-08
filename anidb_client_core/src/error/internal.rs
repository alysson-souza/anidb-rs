//! Internal library error types

use thiserror::Error;

/// Internal library errors
#[derive(Error, Debug)]
pub enum InternalError {
    /// Hash calculation error
    #[error("Hash calculation failed for algorithm '{algorithm}': {message}")]
    HashCalculation { algorithm: String, message: String },

    /// FFI operation error
    #[error("FFI function '{function}' failed: {message}")]
    Ffi { function: String, message: String },

    /// Memory limit exceeded
    #[error(
        "Memory limit exceeded: current usage {current} bytes would exceed limit of {limit} bytes"
    )]
    MemoryLimitExceeded { limit: usize, current: usize },

    /// Unsupported I/O strategy
    #[error("Unsupported I/O strategy '{strategy}': {reason}")]
    UnsupportedIoStrategy { strategy: String, reason: String },

    /// Buffer pool error
    #[error("Buffer pool error: {message}")]
    BufferPool { message: String },

    /// Internal assertion failure
    #[error("Internal assertion failed: {message}")]
    Assertion { message: String },
}

impl InternalError {
    /// Create a hash calculation error
    pub fn hash_calculation(algorithm: &str, message: &str) -> Self {
        Self::HashCalculation {
            algorithm: algorithm.to_string(),
            message: message.to_string(),
        }
    }

    /// Create an FFI error
    pub fn ffi(function: &str, message: &str) -> Self {
        Self::Ffi {
            function: function.to_string(),
            message: message.to_string(),
        }
    }

    /// Create a memory limit exceeded error
    pub fn memory_limit_exceeded(limit: usize, current: usize) -> Self {
        Self::MemoryLimitExceeded { limit, current }
    }

    /// Create an unsupported I/O strategy error
    pub fn unsupported_io_strategy(strategy: &str, reason: &str) -> Self {
        Self::UnsupportedIoStrategy {
            strategy: strategy.to_string(),
            reason: reason.to_string(),
        }
    }

    /// Create a buffer pool error
    pub fn buffer_pool(message: impl Into<String>) -> Self {
        Self::BufferPool {
            message: message.into(),
        }
    }

    /// Create an internal assertion failure error
    pub fn assertion(message: impl Into<String>) -> Self {
        Self::Assertion {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::MemoryLimitExceeded { .. } | Self::BufferPool { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_calculation_error() {
        let error = InternalError::hash_calculation("ED2K", "Chunk processing failed");
        assert!(error.to_string().contains("Hash calculation failed"));
        assert!(error.to_string().contains("ED2K"));
        assert!(error.to_string().contains("Chunk processing failed"));
    }

    #[test]
    fn test_ffi_error() {
        let error = InternalError::ffi("anidb_process_file", "Invalid handle");
        assert!(error.to_string().contains("FFI function"));
        assert!(error.to_string().contains("anidb_process_file"));
        assert!(error.to_string().contains("Invalid handle"));
    }

    #[test]
    fn test_memory_limit_exceeded_error() {
        let error = InternalError::memory_limit_exceeded(500_000_000, 600_000_000);
        assert!(error.to_string().contains("Memory limit exceeded"));
        assert!(error.to_string().contains("500000000"));
        assert!(error.to_string().contains("600000000"));
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_unsupported_io_strategy_error() {
        let error = InternalError::unsupported_io_strategy("mmap", "Not available on Windows");
        assert!(error.to_string().contains("Unsupported I/O strategy"));
        assert!(error.to_string().contains("mmap"));
        assert!(error.to_string().contains("Not available on Windows"));
    }

    #[test]
    fn test_buffer_pool_error() {
        let error = InternalError::buffer_pool("All buffers in use");
        assert!(error.to_string().contains("Buffer pool error"));
        assert!(error.to_string().contains("All buffers in use"));
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_assertion_error() {
        let error = InternalError::assertion("Invariant violated");
        assert!(error.to_string().contains("Internal assertion failed"));
        assert!(error.to_string().contains("Invariant violated"));
        assert!(!error.is_recoverable());
    }
}
