//! Progress reporting stage for the streaming pipeline
//!
//! This stage reports progress updates as data flows through the pipeline.

use super::ProcessingStage;
use crate::progress::{ProgressProvider, ProgressUpdate};
use crate::{Error, Result, error::ValidationError};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Stage that reports progress during processing
pub struct ProgressStage {
    /// Progress provider for reporting
    provider: Arc<dyn ProgressProvider>,
    /// Path being processed (for reporting)
    file_path: PathBuf,
    /// Total size of the file
    total_size: u64,
    /// Bytes processed so far
    bytes_processed: u64,
    /// Start time for throughput calculation
    start_time: Option<Instant>,
    /// Operation description
    operation: String,
    /// Report interval (report every N bytes)
    report_interval: u64,
    /// Bytes since last report
    bytes_since_report: u64,
}

impl std::fmt::Debug for ProgressStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressStage")
            .field("file_path", &self.file_path)
            .field("total_size", &self.total_size)
            .field("bytes_processed", &self.bytes_processed)
            .field("operation", &self.operation)
            .field("report_interval", &self.report_interval)
            .finish()
    }
}

impl ProgressStage {
    /// Create a new progress stage
    pub fn new(provider: Arc<dyn ProgressProvider>, file_path: PathBuf, operation: String) -> Self {
        Self {
            provider,
            file_path,
            total_size: 0,
            bytes_processed: 0,
            start_time: None,
            operation,
            report_interval: 1024 * 1024, // Report every 1MB by default
            bytes_since_report: 0,
        }
    }

    /// Set the reporting interval in bytes
    pub fn with_report_interval(mut self, interval: u64) -> Self {
        self.report_interval = interval;
        self
    }

    /// Calculate current throughput in MB/s
    fn calculate_throughput(&self) -> Option<f64> {
        self.start_time.map(|start| {
            let elapsed = start.elapsed();
            if elapsed.as_secs_f64() > 0.0 {
                (self.bytes_processed as f64 / elapsed.as_secs_f64()) / (1024.0 * 1024.0)
            } else {
                0.0
            }
        })
    }

    /// Send a progress update
    fn send_update(&self) {
        let update = ProgressUpdate::FileProgress {
            path: self.file_path.clone(),
            bytes_processed: self.bytes_processed,
            total_bytes: self.total_size,
            operation: self.operation.clone(),
            throughput_mbps: self.calculate_throughput(),
            memory_usage_bytes: None,
            buffer_size: None,
        };

        self.provider.report(update);
    }
}

#[async_trait]
impl ProcessingStage for ProgressStage {
    async fn process(&mut self, chunk: &[u8]) -> Result<()> {
        let chunk_size = chunk.len() as u64;
        self.bytes_processed += chunk_size;
        self.bytes_since_report += chunk_size;

        // Report progress if we've processed enough bytes
        if self.bytes_since_report >= self.report_interval {
            self.send_update();
            self.bytes_since_report = 0;
        }

        Ok(())
    }

    async fn initialize(&mut self, total_size: u64) -> Result<()> {
        self.total_size = total_size;
        self.bytes_processed = 0;
        self.bytes_since_report = 0;
        self.start_time = Some(Instant::now());

        // Send initial progress
        self.send_update();

        Ok(())
    }

    async fn finalize(&mut self) -> Result<()> {
        // Send final progress update
        self.send_update();

        // Note: provider.complete() is called by FileProcessor after extracting results
        // to avoid deadlock with HashingStage finalization

        Ok(())
    }

    fn name(&self) -> &str {
        "ProgressStage"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

/// Builder for ProgressStage
#[allow(dead_code)]
pub struct ProgressStageBuilder {
    provider: Option<Arc<dyn ProgressProvider>>,
    file_path: Option<PathBuf>,
    operation: String,
    report_interval: u64,
}

#[allow(dead_code)]
impl ProgressStageBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            provider: None,
            file_path: None,
            operation: "Processing".to_string(),
            report_interval: 1024 * 1024, // 1MB default
        }
    }

    /// Set the progress provider
    pub fn with_provider(mut self, provider: Arc<dyn ProgressProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Set the file path being processed
    pub fn with_file_path(mut self, path: PathBuf) -> Self {
        self.file_path = Some(path);
        self
    }

    /// Set the operation description
    pub fn with_operation(mut self, operation: String) -> Self {
        self.operation = operation;
        self
    }

    /// Set the report interval
    pub fn with_report_interval(mut self, interval: u64) -> Self {
        self.report_interval = interval;
        self
    }

    /// Build the progress stage
    pub fn build(self) -> Result<ProgressStage> {
        let provider = self.provider.ok_or_else(|| {
            Error::Validation(ValidationError::invalid_configuration(
                "ProgressStage requires a provider",
            ))
        })?;
        let file_path = self.file_path.ok_or_else(|| {
            Error::Validation(ValidationError::invalid_configuration(
                "ProgressStage requires a file path",
            ))
        })?;

        Ok(ProgressStage {
            provider,
            file_path,
            total_size: 0,
            bytes_processed: 0,
            start_time: None,
            operation: self.operation,
            report_interval: self.report_interval,
            bytes_since_report: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::NullProvider;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingProvider {
        count: Arc<AtomicUsize>,
    }

    impl ProgressProvider for CountingProvider {
        fn report(&self, _update: ProgressUpdate) {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
            Box::new(CountingProvider {
                count: Arc::clone(&self.count),
            })
        }

        fn complete(&self) {}
    }

    #[tokio::test]
    async fn test_progress_stage_reporting() {
        let count = Arc::new(AtomicUsize::new(0));
        let provider = Arc::new(CountingProvider {
            count: Arc::clone(&count),
        });

        let mut stage = ProgressStage::new(
            provider,
            PathBuf::from("/test/file.mkv"),
            "Testing".to_string(),
        )
        .with_report_interval(10); // Report every 10 bytes

        // Initialize
        stage.initialize(100).await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1); // Initial report

        // Process chunks
        stage.process(&[0; 5]).await.unwrap(); // 5 bytes, no report
        assert_eq!(count.load(Ordering::SeqCst), 1);

        stage.process(&[0; 6]).await.unwrap(); // 11 bytes total, should report
        assert_eq!(count.load(Ordering::SeqCst), 2);

        // Finalize
        stage.finalize().await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 3); // Final report
    }

    #[tokio::test]
    async fn test_progress_stage_throughput() {
        let provider = Arc::new(NullProvider);
        let mut stage = ProgressStage::new(
            provider,
            PathBuf::from("/test/file.mkv"),
            "Testing".to_string(),
        );

        // Initialize
        stage.initialize(1000).await.unwrap();

        // Process some data
        stage.process(&[0; 100]).await.unwrap();

        // Throughput should be calculated
        let throughput = stage.calculate_throughput();
        assert!(throughput.is_some());
    }

    #[test]
    fn test_builder() {
        let provider = Arc::new(NullProvider);
        let result = ProgressStageBuilder::new()
            .with_provider(provider)
            .with_file_path(PathBuf::from("/test.mkv"))
            .with_operation("Hashing".to_string())
            .with_report_interval(2048)
            .build();

        assert!(result.is_ok());
        let stage = result.unwrap();
        assert_eq!(stage.operation, "Hashing");
        assert_eq!(stage.report_interval, 2048);
    }
}
