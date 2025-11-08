//! Benchmark to measure actual parallel hashing performance improvements
//!
//! Run with: cargo run --example benchmark_parallel_performance --release

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use tokio::runtime::Runtime;

fn create_test_file(path: &Path, size: usize) -> std::io::Result<()> {
    let mut file = File::create(path)?;
    let chunk_size = 1024 * 1024; // 1MB chunks
    let chunk = vec![0x42u8; chunk_size];
    let chunks = size / chunk_size;
    let remainder = size % chunk_size;

    for _ in 0..chunks {
        file.write_all(&chunk)?;
    }

    if remainder > 0 {
        file.write_all(&chunk[..remainder])?;
    }

    file.flush()?;
    Ok(())
}

async fn benchmark_algorithms(file_path: &Path, algorithms: &[HashAlgorithm], method: &str) -> f64 {
    let calculator = HashCalculator::new();
    let start = Instant::now();

    let results = match method {
        "sequential" => calculator.calculate_multiple(file_path, algorithms).await,
        "broadcast" => calculator.calculate_parallel(file_path, algorithms).await,
        "hybrid" => {
            calculator
                .calculate_true_parallel(file_path, algorithms)
                .await
        }
        _ => panic!("Unknown method"),
    };

    results.expect("Hash calculation failed");

    let elapsed = start.elapsed();
    let file_size = std::fs::metadata(file_path).unwrap().len();

    (file_size as f64 / 1_048_576.0) / elapsed.as_secs_f64()
}

fn main() {
    let rt = Runtime::new().unwrap();

    // Create test files
    let sizes = vec![
        ("100MB", 100 * 1024 * 1024),
        ("500MB", 500 * 1024 * 1024),
        ("1GB", 1024 * 1024 * 1024),
    ];

    println!("Creating test files...");
    for (name, size) in &sizes {
        let path = format!("/tmp/test_file_{name}.bin");
        if !Path::new(&path).exists() {
            create_test_file(Path::new(&path), *size).expect("Failed to create test file");
        }
    }

    // Test different algorithm combinations
    let test_cases = vec![
        (
            "All algorithms",
            vec![
                HashAlgorithm::ED2K,
                HashAlgorithm::CRC32,
                HashAlgorithm::TTH,
                HashAlgorithm::SHA1,
                HashAlgorithm::MD5,
            ],
        ),
        (
            "Fast algorithms only",
            vec![HashAlgorithm::CRC32, HashAlgorithm::TTH],
        ),
        (
            "With ED2K bottleneck",
            vec![HashAlgorithm::ED2K, HashAlgorithm::TTH],
        ),
    ];

    println!(
        "\n{:<25} {:<15} {:<15} {:<15} {:<15}",
        "Test Case", "File Size", "Sequential", "Broadcast", "Hybrid"
    );
    println!("{}", "-".repeat(85));

    for (test_name, algorithms) in test_cases {
        for (size_name, size) in &sizes {
            let file_path = format!("/tmp/test_file_{size_name}.bin");
            let path = Path::new(&file_path);

            print!("{test_name:<25} {size_name:<15} ");
            std::io::stdout().flush().unwrap();

            // Skip sequential for large files with many algorithms
            let _seq_throughput = if algorithms.len() <= 3 || *size <= 100 * 1024 * 1024 {
                let throughput = rt.block_on(benchmark_algorithms(path, &algorithms, "sequential"));
                print!("{throughput:<15.1} ");
                std::io::stdout().flush().unwrap();
                throughput
            } else {
                print!("{:<15} ", "---");
                std::io::stdout().flush().unwrap();
                0.0
            };

            // Benchmark broadcast parallel
            let broadcast_throughput =
                rt.block_on(benchmark_algorithms(path, &algorithms, "broadcast"));
            print!("{broadcast_throughput:<15.1} ");
            std::io::stdout().flush().unwrap();

            // Benchmark hybrid parallel
            let hybrid_throughput = rt.block_on(benchmark_algorithms(path, &algorithms, "hybrid"));
            let improvement = if broadcast_throughput > 0.0 {
                ((hybrid_throughput - broadcast_throughput) / broadcast_throughput) * 100.0
            } else {
                0.0
            };

            println!("{hybrid_throughput:<15.1} ({improvement:+.1}%)");
        }
        println!();
    }

    // Individual algorithm speeds
    println!("\nIndividual Algorithm Speeds (100MB file):");
    println!("{:<15} {:<15}", "Algorithm", "Throughput MB/s");
    println!("{}", "-".repeat(30));

    let file_path = "/tmp/test_file_100MB.bin";
    let path = Path::new(file_path);

    for algo in &[
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::TTH,
        HashAlgorithm::SHA1,
        HashAlgorithm::MD5,
    ] {
        let throughput = rt.block_on(benchmark_algorithms(path, &[*algo], "sequential"));
        println!("{:<15} {:<15.1}", format!("{:?}", algo), throughput);
    }

    // Cleanup
    println!("\nCleaning up test files...");
    for (name, _) in &sizes {
        let path = format!("/tmp/test_file_{name}.bin");
        let _ = std::fs::remove_file(&path);
    }
}
