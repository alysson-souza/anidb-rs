//! Validation stage for the streaming pipeline
//!
//! This stage validates data chunks and enforces constraints.

use super::ProcessingStage;
use crate::{Error, Result};
use async_trait::async_trait;

/// Stage that validates data during processing
#[derive(Debug)]
pub struct ValidationStage {
    /// Maximum file size allowed (0 = no limit)
    max_file_size: u64,
    /// Minimum file size required (0 = no minimum)
    min_file_size: u64,
    /// Maximum chunk size allowed
    max_chunk_size: usize,
    /// Total bytes seen so far
    total_bytes: u64,
    /// Whether to check for empty chunks
    reject_empty_chunks: bool,
    /// Statistics
    chunks_validated: usize,
    empty_chunks_seen: usize,
}

impl Default for ValidationStage {
    fn default() -> Self {
        Self {
            max_file_size: 0, // No limit by default
            min_file_size: 0,
            max_chunk_size: 10 * 1024 * 1024, // 10MB chunks max
            total_bytes: 0,
            reject_empty_chunks: false,
            chunks_validated: 0,
            empty_chunks_seen: 0,
        }
    }
}

impl ValidationStage {
    /// Create a new validation stage with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum file size constraint
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set minimum file size constraint
    pub fn with_min_file_size(mut self, size: u64) -> Self {
        self.min_file_size = size;
        self
    }

    /// Set maximum chunk size
    pub fn with_max_chunk_size(mut self, size: usize) -> Self {
        self.max_chunk_size = size;
        self
    }

    /// Set whether to reject empty chunks
    pub fn reject_empty_chunks(mut self, reject: bool) -> Self {
        self.reject_empty_chunks = reject;
        self
    }

    /// Get validation statistics
    pub fn stats(&self) -> ValidationStats {
        ValidationStats {
            chunks_validated: self.chunks_validated,
            empty_chunks_seen: self.empty_chunks_seen,
            total_bytes: self.total_bytes,
        }
    }
}

#[async_trait]
impl ProcessingStage for ValidationStage {
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        // Check empty chunks
        if chunk.is_empty() {
            self.empty_chunks_seen += 1;
            if self.reject_empty_chunks {
                return Err(Error::Validation(
                    crate::error::ValidationError::InvalidConfiguration {
                        message: "Empty chunk not allowed".to_string(),
                    },
                ));
            }
        }

        // Check chunk size
        if chunk.len() > self.max_chunk_size {
            return Err(Error::Validation(
                crate::error::ValidationError::InvalidConfiguration {
                    message: format!(
                        "Chunk size {} exceeds maximum {}",
                        chunk.len(),
                        self.max_chunk_size
                    ),
                },
            ));
        }

        // Update total and check file size
        self.total_bytes += chunk.len() as u64;
        if self.max_file_size > 0 && self.total_bytes > self.max_file_size {
            return Err(Error::Validation(
                crate::error::ValidationError::InvalidConfiguration {
                    message: format!(
                        "File size {} exceeds maximum {}",
                        self.total_bytes, self.max_file_size
                    ),
                },
            ));
        }

        self.chunks_validated += 1;
        Ok(())
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        // Reset counters
        self.total_bytes = 0;
        self.chunks_validated = 0;
        self.empty_chunks_seen = 0;

        // Validate initial size constraints
        if self.max_file_size > 0 && total_size > self.max_file_size {
            return Err(Error::Validation(
                crate::error::ValidationError::InvalidConfiguration {
                    message: format!(
                        "File size {} exceeds maximum {}",
                        total_size, self.max_file_size
                    ),
                },
            ));
        }

        if self.min_file_size > 0 && total_size < self.min_file_size {
            return Err(Error::Validation(
                crate::error::ValidationError::InvalidConfiguration {
                    message: format!(
                        "File size {} below minimum {}",
                        total_size, self.min_file_size
                    ),
                },
            ));
        }

        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        // Final validation
        if self.min_file_size > 0 && self.total_bytes < self.min_file_size {
            return Err(Error::Validation(
                crate::error::ValidationError::InvalidConfiguration {
                    message: format!(
                        "Total processed {} below minimum {}",
                        self.total_bytes, self.min_file_size
                    ),
                },
            ));
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "ValidationStage"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// Statistics from validation
#[derive(Debug, Clone)]
pub struct ValidationStats {
    pub chunks_validated: usize,
    pub empty_chunks_seen: usize,
    pub total_bytes: u64,
}

/// Builder for ValidationStage
#[allow(dead_code)]
pub struct ValidationStageBuilder {
    max_file_size: u64,
    min_file_size: u64,
    max_chunk_size: usize,
    reject_empty_chunks: bool,
}

impl Default for ValidationStageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl ValidationStageBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            max_file_size: 0,
            min_file_size: 0,
            max_chunk_size: 10 * 1024 * 1024,
            reject_empty_chunks: false,
        }
    }

    /// Set maximum file size
    pub fn max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set minimum file size
    pub fn min_file_size(mut self, size: u64) -> Self {
        self.min_file_size = size;
        self
    }

    /// Set maximum chunk size
    pub fn max_chunk_size(mut self, size: usize) -> Self {
        self.max_chunk_size = size;
        self
    }

    /// Set whether to reject empty chunks
    pub fn reject_empty_chunks(mut self, reject: bool) -> Self {
        self.reject_empty_chunks = reject;
        self
    }

    /// Build the validation stage
    pub fn build(self) -> ValidationStage {
        ValidationStage {
            max_file_size: self.max_file_size,
            min_file_size: self.min_file_size,
            max_chunk_size: self.max_chunk_size,
            total_bytes: 0,
            reject_empty_chunks: self.reject_empty_chunks,
            chunks_validated: 0,
            empty_chunks_seen: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validation_chunk_size() {
        let mut stage = ValidationStage::new().with_max_chunk_size(10);

        // Small chunk should pass
        assert!(stage.process(&[0; 5]).await.is_ok());

        // Large chunk should fail
        assert!(stage.process(&[0; 20]).await.is_err());
    }

    #[tokio::test]
    async fn test_validation_file_size() {
        let mut stage = ValidationStage::new().with_max_file_size(100);

        // Initialize with size under limit
        assert!(stage.initialize(50).await.is_ok());

        // Process data
        assert!(stage.process(&[0; 30]).await.is_ok());
        assert!(stage.process(&[0; 40]).await.is_ok());

        // This should exceed the limit
        let result = stage.process(&[0; 50]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validation_empty_chunks() {
        let mut stage = ValidationStage::new().reject_empty_chunks(true);

        // Non-empty chunk should pass
        assert!(stage.process(&[1, 2, 3]).await.is_ok());

        // Empty chunk should fail
        assert!(stage.process(&[]).await.is_err());
    }

    #[tokio::test]
    async fn test_validation_minimum_size() {
        let mut stage = ValidationStage::new().with_min_file_size(100);

        // Initialize should fail if declared size is too small
        assert!(stage.initialize(50).await.is_err());

        // Initialize with adequate size
        assert!(stage.initialize(150).await.is_ok());

        // Process some data
        assert!(stage.process(&[0; 50]).await.is_ok());

        // Finalize should fail if we didn't process enough
        assert!(stage.finalize().await.is_err());

        // Process more to meet minimum
        assert!(stage.process(&[0; 60]).await.is_ok());
        assert!(stage.finalize().await.is_ok());
    }

    #[tokio::test]
    async fn test_validation_stats() {
        let mut stage = ValidationStage::new();

        stage.initialize(100).await.unwrap();
        stage.process(&[0; 10]).await.unwrap();
        stage.process(&[]).await.unwrap(); // Empty chunk
        stage.process(&[0; 20]).await.unwrap();

        let stats = stage.stats();
        assert_eq!(stats.chunks_validated, 3);
        assert_eq!(stats.empty_chunks_seen, 1);
        assert_eq!(stats.total_bytes, 30);
    }

    #[test]
    fn test_builder() {
        let stage = ValidationStageBuilder::new()
            .max_file_size(1000)
            .min_file_size(10)
            .max_chunk_size(100)
            .reject_empty_chunks(true)
            .build();

        assert_eq!(stage.max_file_size, 1000);
        assert_eq!(stage.min_file_size, 10);
        assert_eq!(stage.max_chunk_size, 100);
        assert!(stage.reject_empty_chunks);
    }
}
