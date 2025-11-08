//! Progress reporting abstractions for AniDB client
//!
//! This module provides a trait-based abstraction for progress reporting,
//! allowing the core library to report progress without depending on
//! specific channel implementations or UI concerns.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Core trait for progress reporting
///
/// This trait abstracts away the progress reporting mechanism,
/// allowing different implementations (channels, logging, null, etc.)
pub trait ProgressProvider: Send + Sync {
    /// Report a progress update
    fn report(&self, update: ProgressUpdate);

    /// Create a child provider for nested operations
    fn create_child(&self, name: &str) -> Box<dyn ProgressProvider>;

    /// Signal that the operation is complete
    fn complete(&self);
}

/// Unified progress update type
#[derive(Debug, Clone)]
pub enum ProgressUpdate {
    /// File processing progress
    FileProgress {
        path: PathBuf,
        bytes_processed: u64,
        total_bytes: u64,
        operation: String,
        throughput_mbps: Option<f64>,
        memory_usage_bytes: Option<u64>,
        buffer_size: Option<usize>,
    },

    /// Network operation progress
    NetworkProgress { operation: String, status: String },

    /// Batch operation progress
    BatchProgress {
        current: usize,
        total: usize,
        current_file: Option<String>,
    },

    /// Hash calculation progress
    HashProgress {
        algorithm: String,
        bytes_processed: u64,
        total_bytes: u64,
    },

    /// Generic status message
    Status { message: String },
}

/// Null implementation for when no progress is needed
pub struct NullProvider;

impl ProgressProvider for NullProvider {
    fn report(&self, _update: ProgressUpdate) {
        // No-op: discard all progress updates
    }

    fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
        Box::new(NullProvider)
    }

    fn complete(&self) {
        // No-op
    }
}

/// Arc-wrapped provider for easy sharing across async tasks
pub struct SharedProvider {
    inner: Arc<dyn ProgressProvider>,
}

impl SharedProvider {
    /// Create a new shared provider wrapping the given provider
    pub fn new(provider: Arc<dyn ProgressProvider>) -> Self {
        Self { inner: provider }
    }
}

impl ProgressProvider for SharedProvider {
    fn report(&self, update: ProgressUpdate) {
        self.inner.report(update);
    }

    fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
        Box::new(SharedProvider {
            inner: Arc::clone(&self.inner),
        })
    }

    fn complete(&self) {
        self.inner.complete();
    }
}

/// Helper functions for creating providers
impl dyn ProgressProvider {
    /// Create a null provider (useful for tests and when progress isn't needed)
    pub fn null() -> Box<dyn ProgressProvider> {
        Box::new(NullProvider)
    }
}

/// Convert from the old Progress type to the new ProgressUpdate
/// This is a temporary conversion to help with migration
impl From<crate::Progress> for ProgressUpdate {
    fn from(progress: crate::Progress) -> Self {
        ProgressUpdate::FileProgress {
            path: PathBuf::new(), // Will need to be provided separately
            bytes_processed: progress.bytes_processed,
            total_bytes: progress.total_bytes,
            operation: progress.current_operation,
            throughput_mbps: Some(progress.throughput_mbps),
            memory_usage_bytes: progress.memory_usage_bytes,
            buffer_size: progress.buffer_size,
        }
    }
}

/// Adapter that converts old channel-based progress to the new ProgressProvider trait
pub struct ChannelAdapter {
    tx: Mutex<Option<tokio::sync::mpsc::Sender<crate::Progress>>>,
    current_path: Option<PathBuf>,
    start_time: Instant,
}

impl ChannelAdapter {
    /// Create a new adapter from an mpsc sender
    pub fn new(tx: tokio::sync::mpsc::Sender<crate::Progress>) -> Self {
        Self {
            tx: Mutex::new(Some(tx)),
            current_path: None,
            start_time: Instant::now(),
        }
    }

    /// Set the current file path for progress reporting
    pub fn with_path(mut self, path: PathBuf) -> Self {
        self.current_path = Some(path);
        self
    }
}

impl ProgressProvider for ChannelAdapter {
    fn report(&self, update: ProgressUpdate) {
        // Convert ProgressUpdate back to Progress for the old system
        let progress = match update {
            ProgressUpdate::FileProgress {
                bytes_processed,
                total_bytes,
                operation,
                throughput_mbps,
                memory_usage_bytes,
                buffer_size,
                ..
            } => {
                let percentage = if total_bytes > 0 {
                    (bytes_processed as f64 / total_bytes as f64) * 100.0
                } else {
                    0.0
                };

                let elapsed = self.start_time.elapsed().as_secs_f64();
                let throughput = if elapsed > 0.0 {
                    (bytes_processed as f64 / 1_048_576.0) / elapsed
                } else {
                    0.0
                };
                crate::Progress {
                    percentage,
                    bytes_processed,
                    total_bytes,
                    throughput_mbps: throughput_mbps.unwrap_or(throughput),
                    current_operation: operation,
                    memory_usage_bytes,
                    peak_memory_bytes: None,
                    buffer_size,
                }
            }
            ProgressUpdate::HashProgress {
                algorithm,
                bytes_processed,
                total_bytes,
            } => {
                let percentage = if total_bytes > 0 {
                    (bytes_processed as f64 / total_bytes as f64) * 100.0
                } else {
                    0.0
                };
                let elapsed = self.start_time.elapsed().as_secs_f64();
                let throughput = if elapsed > 0.0 {
                    (bytes_processed as f64 / 1_048_576.0) / elapsed
                } else {
                    0.0
                };
                crate::Progress {
                    percentage,
                    bytes_processed,
                    total_bytes,
                    throughput_mbps: throughput,
                    current_operation: format!("Hashing with {algorithm}"),
                    memory_usage_bytes: None,
                    peak_memory_bytes: None,
                    buffer_size: None,
                }
            }
            ProgressUpdate::Status { message } => crate::Progress {
                percentage: 0.0,
                bytes_processed: 0,
                total_bytes: 0,
                throughput_mbps: 0.0,
                current_operation: message,
                memory_usage_bytes: None,
                peak_memory_bytes: None,
                buffer_size: None,
            },
            _ => return, // Ignore other update types for now
        };

        // Try to send, but ignore errors (receiver might be dropped)
        let tx_opt = { self.tx.lock().unwrap().clone() };
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(progress);
        }
    }

    fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
        let tx_opt = { self.tx.lock().unwrap().clone() };
        Box::new(ChannelAdapter {
            tx: Mutex::new(tx_opt),
            current_path: self.current_path.clone(),
            start_time: self.start_time,
        })
    }

    fn complete(&self) {
        // Drop our sender to allow receivers to detect channel closure
        let mut guard = self.tx.lock().unwrap();
        *guard = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Test provider that captures progress updates
    struct TestProvider {
        updates: Arc<Mutex<Vec<ProgressUpdate>>>,
    }

    impl TestProvider {
        fn new() -> Self {
            Self {
                updates: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn received_updates(&self) -> usize {
            self.updates.lock().unwrap().len()
        }

        fn get_updates(&self) -> Vec<ProgressUpdate> {
            self.updates.lock().unwrap().clone()
        }
    }

    impl ProgressProvider for TestProvider {
        fn report(&self, update: ProgressUpdate) {
            self.updates.lock().unwrap().push(update);
        }

        fn create_child(&self, _name: &str) -> Box<dyn ProgressProvider> {
            Box::new(TestProvider {
                updates: Arc::clone(&self.updates),
            })
        }

        fn complete(&self) {
            // Could track completions if needed
        }
    }

    #[test]
    fn test_null_provider() {
        let provider = NullProvider;

        // Should not panic when reporting
        provider.report(ProgressUpdate::Status {
            message: "Test".to_string(),
        });

        // Should create child without issues
        let child = provider.create_child("test");
        child.report(ProgressUpdate::Status {
            message: "Child test".to_string(),
        });

        provider.complete();
    }

    #[test]
    fn test_test_provider() {
        let provider = TestProvider::new();

        provider.report(ProgressUpdate::Status {
            message: "Test 1".to_string(),
        });

        provider.report(ProgressUpdate::FileProgress {
            path: PathBuf::from("/test/file.txt"),
            bytes_processed: 1024,
            total_bytes: 2048,
            operation: "Hashing".to_string(),
            throughput_mbps: Some(100.0),
            memory_usage_bytes: Some(1024 * 1024),
            buffer_size: Some(65536),
        });

        assert_eq!(provider.received_updates(), 2);

        let updates = provider.get_updates();
        assert_eq!(updates.len(), 2);
    }

    #[test]
    fn test_shared_provider() {
        let test_provider = Arc::new(TestProvider::new());
        let shared = SharedProvider::new(test_provider.clone());

        shared.report(ProgressUpdate::Status {
            message: "Shared test".to_string(),
        });

        let child = shared.create_child("child");
        child.report(ProgressUpdate::Status {
            message: "Child message".to_string(),
        });

        assert_eq!(test_provider.received_updates(), 2);
    }
}
