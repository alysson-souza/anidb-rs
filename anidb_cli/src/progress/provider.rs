//! Progress provider implementation for CLI
//!
//! This module provides the CLI-specific implementation of the ProgressProvider trait,
//! which bridges the core library's progress reporting with the CLI's rendering system.

use anidb_client_core::progress::{ProgressProvider, ProgressUpdate};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Channel-based progress provider for CLI rendering
///
/// This provider sends progress updates through a channel to a separate
/// rendering task, decoupling the progress reporting from the UI rendering.
pub struct ChannelProvider {
    tx: Mutex<Option<mpsc::Sender<ProgressUpdate>>>,
    name: Option<String>,
}

impl ChannelProvider {
    /// Create a new channel provider
    #[allow(dead_code)]
    pub fn new(tx: mpsc::Sender<ProgressUpdate>) -> Self {
        Self {
            tx: Mutex::new(Some(tx)),
            name: None,
        }
    }

    /// Create a new channel provider with a name
    pub fn with_name(tx: mpsc::Sender<ProgressUpdate>, name: String) -> Self {
        Self {
            tx: Mutex::new(Some(tx)),
            name: Some(name),
        }
    }
}

impl ProgressProvider for ChannelProvider {
    fn report(&self, update: ProgressUpdate) {
        // Add context if we have a name
        let update = if let Some(ref name) = self.name {
            match update {
                ProgressUpdate::Status { message } => ProgressUpdate::Status {
                    message: format!("[{name}] {message}"),
                },
                other => other,
            }
        } else {
            update
        };

        // Try to send, but don't block or panic if receiver is dropped
        let tx_opt = { self.tx.lock().unwrap().clone() };
        if let Some(tx) = tx_opt {
            let _ = tx.try_send(update);
        }
    }

    fn create_child(&self, name: &str) -> Box<dyn ProgressProvider> {
        let tx_opt = { self.tx.lock().unwrap().clone() };
        let child_name = if let Some(ref parent_name) = self.name {
            format!("{parent_name}/{name}")
        } else {
            name.to_string()
        };
        match tx_opt {
            Some(tx) => Box::new(ChannelProvider::with_name(tx, child_name)),
            None => Box::new(ChannelProvider {
                tx: Mutex::new(None),
                name: Some(child_name),
            }),
        }
    }

    fn complete(&self) {
        // Drop our sender so the renderer can exit its loop
        let mut guard = self.tx.lock().unwrap();
        *guard = None;
    }
}

/// Create a progress provider and renderer pair for CLI operations
#[allow(dead_code)]
pub fn create_progress_infrastructure()
-> (Arc<dyn ProgressProvider>, mpsc::Receiver<ProgressUpdate>) {
    let (tx, rx) = mpsc::channel(100);
    let provider = Arc::new(ChannelProvider::new(tx)) as Arc<dyn ProgressProvider>;
    (provider, rx)
}

/// Helper to convert old Progress to ProgressUpdate for rendering
#[allow(dead_code)]
pub fn convert_legacy_progress(
    progress: anidb_client_core::Progress,
    path: Option<std::path::PathBuf>,
) -> ProgressUpdate {
    ProgressUpdate::FileProgress {
        path: path.unwrap_or_default(),
        bytes_processed: progress.bytes_processed,
        total_bytes: progress.total_bytes,
        operation: progress.current_operation,
        throughput_mbps: Some(progress.throughput_mbps),
        memory_usage_bytes: progress.memory_usage_bytes,
        buffer_size: progress.buffer_size,
    }
}
