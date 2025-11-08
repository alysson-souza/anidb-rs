//! Demonstration of adaptive buffer sizing in action

use anidb_client_core::progress::{ChannelAdapter, ProgressProvider};
use anidb_client_core::{AniDBClient, ClientConfig, HashAlgorithm, ProcessOptions, Progress};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Skip logging initialization in example

    // Create client
    let config = ClientConfig::default();
    let client = AniDBClient::new(config).await?;

    // Get test file from command line or use default
    let file_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "test_file.mkv".to_string());

    if !Path::new(&file_path).exists() {
        eprintln!("File not found: {file_path}");
        eprintln!("Usage: cargo run --example adaptive_buffer_demo <file_path>");
        return Ok(());
    }

    println!("Processing file: {file_path}");
    println!("Watch for buffer size changes in the progress output!");
    println!("{}", "=".repeat(80));

    // Create progress channel
    let (progress_tx, mut progress_rx) = mpsc::channel::<Progress>(100);

    // Track buffer sizes
    let buffer_sizes = Arc::new(Mutex::new(Vec::new()));
    let buffer_sizes_clone = buffer_sizes.clone();

    // Spawn progress monitoring task
    let progress_task = tokio::spawn(async move {
        let mut last_buffer_size = 0;

        while let Some(progress) = progress_rx.recv().await {
            if let Some(buffer_size) = progress.buffer_size {
                let mut sizes = buffer_sizes_clone.lock().await;
                sizes.push(buffer_size);

                // Print progress
                println!(
                    "Progress: {:.1}% | Speed: {:.1} MB/s | Memory: {:.1} MB | Buffer: {:.1} MB | Op: {}",
                    progress.percentage,
                    progress.throughput_mbps,
                    progress.bytes_processed as f64 / 1024.0 / 1024.0,
                    buffer_size as f64 / 1024.0 / 1024.0,
                    progress.current_operation
                );

                // Highlight buffer size changes
                if buffer_size != last_buffer_size && last_buffer_size != 0 {
                    println!(
                        ">>> BUFFER SIZE CHANGED: {:.1} MB → {:.1} MB",
                        last_buffer_size as f64 / 1024.0 / 1024.0,
                        buffer_size as f64 / 1024.0 / 1024.0
                    );
                }

                last_buffer_size = buffer_size;
            }
        }
    });

    // Process file with progress reporting enabled
    // Create progress provider from channel
    let progress_provider = Arc::new(ChannelAdapter::new(progress_tx)) as Arc<dyn ProgressProvider>;

    let options = ProcessOptions::new()
        .with_algorithms(&[HashAlgorithm::ED2K, HashAlgorithm::CRC32])
        .with_progress_reporting(true)
        .with_progress_provider(progress_provider);

    let result = client.process_file(Path::new(&file_path), options).await?;

    // Wait for progress task to complete
    drop(progress_task);

    println!("{}", "=".repeat(80));
    println!("Processing complete!");
    let ed2k_hash = result.hashes.get(&HashAlgorithm::ED2K).unwrap();
    println!("ED2K: {ed2k_hash}");
    let crc32_hash = result.hashes.get(&HashAlgorithm::CRC32).unwrap();
    println!("CRC32: {crc32_hash}");

    // Show buffer size summary
    let sizes = buffer_sizes.lock().await;
    let unique_sizes: std::collections::HashSet<_> = sizes.iter().cloned().collect();
    println!("\nBuffer size summary:");
    let total_updates = sizes.len();
    println!("- Total updates: {total_updates}");
    let unique_count = unique_sizes.len();
    println!("- Unique buffer sizes: {unique_count}");
    for size in &unique_sizes {
        let mb_size = *size as f64 / 1024.0 / 1024.0;
        println!("  - {mb_size:.1} MB");
    }

    Ok(())
}
