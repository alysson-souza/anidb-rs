//! Progress rendering for the CLI
//!
//! This module handles the visual rendering of progress updates,
//! converting ProgressUpdate messages into terminal output.

use anidb_client_core::progress::ProgressUpdate;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Default)]
struct FileProgressStats {
    throughput_mbps: Option<f64>,
    memory_usage_bytes: Option<u64>,
    buffer_size: Option<usize>,
}

/// Render progress updates from a channel
#[allow(dead_code)]
pub async fn render_progress(mut rx: mpsc::Receiver<ProgressUpdate>) {
    let mut renderer = ProgressRenderer::new();

    while let Some(update) = rx.recv().await {
        renderer.handle_update(update);
    }

    renderer.finish();
}

/// Progress renderer that manages visual progress display
#[allow(dead_code)]
pub struct ProgressRenderer {
    file_bars: HashMap<PathBuf, ProgressBar>,
    network_spinner: Option<ProgressBar>,
    batch_bar: Option<ProgressBar>,
    start_time: Instant,
}

#[allow(dead_code)]
impl ProgressRenderer {
    /// Create a new progress renderer
    pub fn new() -> Self {
        Self {
            file_bars: HashMap::new(),
            network_spinner: None,
            batch_bar: None,
            start_time: Instant::now(),
        }
    }

    /// Handle a progress update
    #[allow(dead_code)]
    pub fn handle_update(&mut self, update: ProgressUpdate) {
        match update {
            ProgressUpdate::FileProgress {
                path,
                bytes_processed,
                total_bytes,
                operation,
                throughput_mbps,
                memory_usage_bytes,
                buffer_size,
            } => {
                let stats = FileProgressStats {
                    throughput_mbps,
                    memory_usage_bytes,
                    buffer_size,
                };
                self.update_file_progress(path, bytes_processed, total_bytes, operation, stats);
            }

            ProgressUpdate::NetworkProgress { operation, status } => {
                self.update_network_progress(operation, status);
            }

            ProgressUpdate::BatchProgress {
                current,
                total,
                current_file,
            } => {
                self.update_batch_progress(current, total, current_file);
            }

            ProgressUpdate::HashProgress {
                algorithm,
                bytes_processed,
                total_bytes,
            } => {
                self.update_hash_progress(algorithm, bytes_processed, total_bytes);
            }

            ProgressUpdate::Status { message } => {
                self.show_status(message);
            }
        }
    }

    /// Update file progress
    fn update_file_progress(
        &mut self,
        path: PathBuf,
        bytes_processed: u64,
        total_bytes: u64,
        operation: String,
        stats: FileProgressStats,
    ) {
        let pb = self.file_bars.entry(path.clone()).or_insert_with(|| {
            let pb = ProgressBar::new(total_bytes);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {percent}% | {bytes}/{total_bytes} | {bytes_per_sec} | ETA: {eta} | {prefix}")
                    .unwrap()
                    .progress_chars("#>-"),
            );

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            pb.set_message(format!(
                "{}: {} [{}]",
                "Processing".bold(),
                file_name.cyan(),
                operation.yellow()
            ));

            pb
        });

        pb.set_position(bytes_processed);

        // Build prefix with memory and throughput info
        let mut prefix_parts = Vec::new();

        if let Some(mbps) = stats.throughput_mbps {
            prefix_parts.push(format!("{mbps:.1} MB/s"));
        }

        if let Some(mem_bytes) = stats.memory_usage_bytes {
            prefix_parts.push(format!("Mem: {:.1}MB", mem_bytes as f64 / 1_048_576.0));
        }

        if let Some(buf_size) = stats.buffer_size {
            prefix_parts.push(format!("Buf: {}KB", buf_size / 1024));
        }

        if !prefix_parts.is_empty() {
            pb.set_prefix(prefix_parts.join(" | "));
        }
    }

    /// Update network progress
    fn update_network_progress(&mut self, operation: String, status: String) {
        if self.network_spinner.is_none() {
            let spinner = ProgressBar::new_spinner();
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                    .template("{spinner:.cyan} {msg}")
                    .unwrap(),
            );
            spinner.enable_steady_tick(Duration::from_millis(100));
            self.network_spinner = Some(spinner);
        }

        if let Some(spinner) = &self.network_spinner {
            spinner.set_message(format!("{}: {}", operation.bold(), status));
        }
    }

    /// Update batch progress
    fn update_batch_progress(
        &mut self,
        current: usize,
        total: usize,
        current_file: Option<String>,
    ) {
        if self.batch_bar.is_none() {
            let bar = ProgressBar::new(total as u64);
            bar.set_style(
                ProgressStyle::default_bar()
                    .template("{msg}\n[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files | {percent}%")
                    .unwrap()
                    .progress_chars("##-"),
            );
            bar.set_message("Processing batch".bold().to_string());
            self.batch_bar = Some(bar);
        }

        if let Some(bar) = &self.batch_bar {
            bar.set_position(current as u64);
            if let Some(file) = current_file {
                bar.set_message(format!("{}: {}", "Processing batch".bold(), file.cyan()));
            }
        }
    }

    /// Update hash progress
    fn update_hash_progress(&mut self, algorithm: String, bytes_processed: u64, total_bytes: u64) {
        // For now, treat this like a file progress with the algorithm as the operation
        let path = PathBuf::from(format!("hash_{}", algorithm.to_lowercase()));
        self.update_file_progress(
            path,
            bytes_processed,
            total_bytes,
            format!("Hashing with {algorithm}"),
            FileProgressStats::default(),
        );
    }

    /// Show a status message
    fn show_status(&self, message: String) {
        eprintln!("{} {}", "→".green(), message);
    }

    /// Finish all progress bars
    pub fn finish(self) {
        for (_, pb) in self.file_bars {
            pb.finish_with_message("✓ Complete".green().to_string());
        }

        if let Some(spinner) = self.network_spinner {
            spinner.finish_and_clear();
        }

        if let Some(bar) = self.batch_bar {
            bar.finish_with_message("✓ Batch complete".green().to_string());
        }
    }
}

impl Default for ProgressRenderer {
    fn default() -> Self {
        Self::new()
    }
}
