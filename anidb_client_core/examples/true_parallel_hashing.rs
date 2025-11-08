//! Example demonstrating true parallel hashing with independent queues

use anidb_client_core::{HashAlgorithm, HashCalculator, Progress};
use std::path::Path;
use std::time::Instant;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: cargo run --example true_parallel_hashing <file_path>");
        std::process::exit(1);
    });

    let path = Path::new(&file_path);
    if !path.exists() {
        eprintln!("Error: File not found: {file_path}");
        std::process::exit(1);
    }

    println!("True Parallel Hashing Example");
    println!("=============================\n");
    println!("File: {}", path.display());

    let metadata = std::fs::metadata(path)?;
    let file_size = metadata.len();
    println!("Size: {} MB\n", file_size / (1024 * 1024));

    let calculator = HashCalculator::new();

    // Select algorithms to compute
    let algorithms = vec![
        HashAlgorithm::MD5,   // Medium speed
        HashAlgorithm::SHA1,  // Slower
        HashAlgorithm::CRC32, // Fast
        HashAlgorithm::ED2K,  // Complex chunking
    ];

    println!("Computing {} hashes in parallel...", algorithms.len());
    println!("Each algorithm processes different chunks simultaneously!\n");

    // Set up progress tracking
    let (progress_tx, mut progress_rx) = mpsc::channel::<Progress>(100);

    // Spawn progress reporter
    let progress_task = tokio::spawn(async move {
        let mut last_percentage = 0.0;

        while let Some(progress) = progress_rx.recv().await {
            if progress.percentage >= last_percentage + 5.0 {
                println!(
                    "[{:>5.1}%] {} - {:.1} MB/s",
                    progress.percentage, progress.current_operation, progress.throughput_mbps
                );
                last_percentage = progress.percentage;
            }
        }
    });

    // Progress tracking is now handled internally
    // The progress channel is no longer used for parallel calculations
    drop(progress_tx);

    // Start timing
    let start = Instant::now();

    // Compute hashes using true parallel processing
    // Note: The old calculate_true_parallel_with_config method is no longer available.
    // Use the standard parallel calculation method instead.
    let results = calculator
        .calculate_true_parallel(path, &algorithms)
        .await?;

    let total_duration = start.elapsed();

    // Wait for progress reporter to finish
    progress_task.await?;

    // Display results
    println!("\nResults:");
    println!("--------");
    for (algo, result) in &results {
        println!(
            "{:?}: {} (computed in {:.2}s)",
            algo,
            result.hash,
            result.duration.as_secs_f64()
        );
    }

    println!("\nStatistics:");
    println!("-----------");
    println!("Total time: {:.2}s", total_duration.as_secs_f64());
    println!(
        "Average throughput: {:.1} MB/s",
        (file_size as f64 / (1024.0 * 1024.0)) / total_duration.as_secs_f64()
    );

    // Memory usage is managed internally by the parallel implementation

    println!("\nKey advantages of true parallel hashing:");
    println!("- Each algorithm works at its own pace");
    println!("- Fast algorithms (CRC32) don't wait for slow ones (SHA1)");
    println!("- Single I/O thread feeds all algorithms efficiently");
    println!("- Bounded memory usage with configurable queue depth");

    Ok(())
}
