//! Simple benchmark to test parallel hashing performance
//!
//! Run with: cargo run --example simple_parallel_benchmark --release

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use tokio::runtime::Runtime;

fn create_test_file(size: usize) -> std::io::Result<()> {
    let path = "/tmp/benchmark_test.bin";
    let mut file = File::create(path)?;
    let chunk = vec![0x42u8; 1024 * 1024]; // 1MB of data

    for _ in 0..(size / chunk.len()) {
        file.write_all(&chunk)?;
    }

    file.flush()?;
    Ok(())
}

async fn run_benchmark(calculator: &HashCalculator, algorithms: &[HashAlgorithm], label: &str) {
    let path = Path::new("/tmp/benchmark_test.bin");
    let file_size = std::fs::metadata(path).unwrap().len();

    println!("\n{} with {} algorithms:", label, algorithms.len());
    for algo in algorithms {
        println!("  - {algo:?}");
    }

    // Test sequential (single algorithm at a time)
    if algorithms.len() == 1 {
        let start = Instant::now();
        let result = calculator.calculate_file(path, algorithms[0]).await;
        match result {
            Ok(_) => {
                let elapsed = start.elapsed();
                let throughput = (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
                println!("  Single algorithm: {throughput:.1} MB/s");
            }
            Err(e) => println!("  Single algorithm failed: {e}"),
        }
    }

    // Test calculate_multiple (sequential processing)
    let start = Instant::now();
    let result = calculator.calculate_multiple(path, algorithms).await;
    match result {
        Ok(_) => {
            let elapsed = start.elapsed();
            let throughput = (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
            println!("  Sequential: {throughput:.1} MB/s");
        }
        Err(e) => println!("  Sequential failed: {e}"),
    }

    // Test calculate_parallel (broadcast)
    let start = Instant::now();
    let result = calculator.calculate_parallel(path, algorithms).await;
    match result {
        Ok(_) => {
            let elapsed = start.elapsed();
            let throughput = (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
            println!("  Broadcast parallel: {throughput:.1} MB/s");
        }
        Err(e) => println!("  Broadcast parallel failed: {e}"),
    }

    // Test calculate_true_parallel (hybrid)
    let start = Instant::now();
    let result = calculator.calculate_true_parallel(path, algorithms).await;
    match result {
        Ok(_) => {
            let elapsed = start.elapsed();
            let throughput = (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64();
            println!("  Hybrid parallel: {throughput:.1} MB/s");
        }
        Err(e) => println!("  Hybrid parallel failed: {e}"),
    }
}

fn main() {
    let rt = Runtime::new().unwrap();

    // Create a 100MB test file
    println!("Creating 100MB test file...");
    create_test_file(100 * 1024 * 1024).expect("Failed to create test file");

    let calculator = HashCalculator::new();

    // Test individual algorithms
    println!("\n=== INDIVIDUAL ALGORITHM SPEEDS ===");
    rt.block_on(run_benchmark(
        &calculator,
        &[HashAlgorithm::ED2K],
        "ED2K only",
    ));
    rt.block_on(run_benchmark(
        &calculator,
        &[HashAlgorithm::TTH],
        "TTH only",
    ));
    rt.block_on(run_benchmark(
        &calculator,
        &[HashAlgorithm::CRC32],
        "CRC32 only",
    ));

    // Test combinations
    println!("\n=== ALGORITHM COMBINATIONS ===");

    // Fast algorithms only
    rt.block_on(run_benchmark(
        &calculator,
        &[HashAlgorithm::CRC32, HashAlgorithm::TTH],
        "Fast algorithms (CRC32, TTH)",
    ));

    // With ED2K bottleneck
    rt.block_on(run_benchmark(
        &calculator,
        &[HashAlgorithm::ED2K, HashAlgorithm::TTH],
        "With ED2K bottleneck",
    ));

    // All algorithms
    rt.block_on(run_benchmark(
        &calculator,
        &[
            HashAlgorithm::ED2K,
            HashAlgorithm::CRC32,
            HashAlgorithm::TTH,
            HashAlgorithm::SHA1,
            HashAlgorithm::MD5,
        ],
        "All algorithms",
    ));

    // Cleanup
    println!("\nCleaning up...");
    let _ = std::fs::remove_file("/tmp/benchmark_test.bin");

    println!("\n=== SUMMARY ===");
    println!("The hybrid parallel implementation allows fast algorithms to run at their");
    println!("full speed without being bottlenecked by slower algorithms like ED2K.");
}
