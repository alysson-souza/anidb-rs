//! Debug test to understand parallel hash performance

use anidb_client_core::ClientConfig;
use anidb_client_core::file_io::FileProcessor;
use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use anidb_client_core::progress::NullProvider;
use tempfile::TempDir;

#[tokio::test]
async fn debug_parallel_hash_performance() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("perf_test.bin");

    // Create a 50MB test file for more accurate measurement
    let test_data = vec![0xFFu8; 50 * 1024 * 1024];
    std::fs::write(&test_file, &test_data).unwrap();

    let algorithms = vec![
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    // Test 1: Direct hash calculator (without FileProcessor overhead)
    let calculator = HashCalculator::new();

    // Single algorithm
    let start = std::time::Instant::now();
    let _result = calculator
        .calculate_file(&test_file, HashAlgorithm::CRC32)
        .await
        .unwrap();
    let single_calc_time = start.elapsed();

    // Multiple algorithms with calculate_multiple
    let start = std::time::Instant::now();
    let _results = calculator
        .calculate_multiple(&test_file, &algorithms)
        .await
        .unwrap();
    let multi_calc_time = start.elapsed();

    let calc_ratio = multi_calc_time.as_secs_f64() / single_calc_time.as_secs_f64();

    println!("=== Direct HashCalculator ===");
    println!("Single algorithm: {single_calc_time:?}");
    println!("Multiple algorithms: {multi_calc_time:?}");
    println!("Ratio: {calc_ratio:.2}x");

    // Test 2: Via FileProcessor
    let config = ClientConfig::test();
    let processor = FileProcessor::new(config);

    // Single algorithm
    let null_provider = std::sync::Arc::new(NullProvider);
    let start = std::time::Instant::now();
    let _result = processor
        .process_file(&test_file, &[HashAlgorithm::CRC32], null_provider.clone())
        .await
        .unwrap();
    let single_proc_time = start.elapsed();

    // Multiple algorithms
    let start = std::time::Instant::now();
    let _results = processor
        .process_file(&test_file, &algorithms, null_provider.clone())
        .await
        .unwrap();
    let multi_proc_time = start.elapsed();

    let proc_ratio = multi_proc_time.as_secs_f64() / single_proc_time.as_secs_f64();

    println!("\n=== Via FileProcessor ===");
    println!("Single algorithm: {single_proc_time:?}");
    println!("Multiple algorithms: {multi_proc_time:?}");
    println!("Ratio: {proc_ratio:.2}x");

    // Test 3: Individual algorithm times
    println!("\n=== Individual Algorithm Times ===");
    for algo in &algorithms {
        let start = std::time::Instant::now();
        let _result = calculator.calculate_file(&test_file, *algo).await.unwrap();
        let time = start.elapsed();
        println!("{algo}: {time:?}");
    }
}
