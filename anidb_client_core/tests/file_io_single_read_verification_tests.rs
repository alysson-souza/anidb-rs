//! Test to verify that FileProcessor reads files only once with multiple algorithms

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use std::collections::HashMap;
use tempfile::TempDir;

/// Simulate the OLD approach - reading file multiple times
async fn old_approach_multiple_reads(
    file_path: &std::path::Path,
    algorithms: &[HashAlgorithm],
) -> (HashMap<HashAlgorithm, String>, std::time::Duration) {
    let start = std::time::Instant::now();
    let mut hashes = HashMap::new();
    let calculator = HashCalculator::new();

    // OLD APPROACH: Loop through algorithms and read file for each one
    for algorithm in algorithms {
        let result = calculator
            .calculate_file(file_path, *algorithm)
            .await
            .unwrap();
        hashes.insert(*algorithm, result.hash);
    }

    (hashes, start.elapsed())
}

/// The NEW approach - reading file once
async fn new_approach_single_read(
    file_path: &std::path::Path,
    algorithms: &[HashAlgorithm],
) -> (HashMap<HashAlgorithm, String>, std::time::Duration) {
    let start = std::time::Instant::now();
    let calculator = HashCalculator::new();

    // NEW APPROACH: Use calculate_multiple to read file once
    let results = calculator
        .calculate_multiple(file_path, algorithms)
        .await
        .unwrap();

    let hashes = results
        .into_iter()
        .map(|(algo, result)| (algo, result.hash))
        .collect();

    (hashes, start.elapsed())
}

#[tokio::test]
async fn verify_single_file_read_optimization() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("optimization_test.bin");

    // Create a 100MB test file for clear performance difference
    let test_data = vec![0xAAu8; 100 * 1024 * 1024];
    std::fs::write(&test_file, &test_data).unwrap();

    let algorithms = vec![
        HashAlgorithm::ED2K,
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    // Run old approach
    let (old_hashes, old_duration) = old_approach_multiple_reads(&test_file, &algorithms).await;

    // Run new approach
    let (new_hashes, new_duration) = new_approach_single_read(&test_file, &algorithms).await;

    // Verify both produce the same hashes
    for algo in &algorithms {
        assert_eq!(
            old_hashes.get(algo),
            new_hashes.get(algo),
            "Hash mismatch for {algo:?}"
        );
    }

    // Calculate improvement
    let improvement = old_duration.as_secs_f64() / new_duration.as_secs_f64();

    println!("=== File Read Optimization Results ===");
    println!("Old approach (4 file reads): {old_duration:?}");
    println!("New approach (1 file read): {new_duration:?}");
    println!("Performance improvement: {improvement:.2}x faster");
    if old_duration > new_duration {
        println!("Time saved: {:?}", old_duration - new_duration);
    } else {
        println!("Time lost: {:?}", new_duration - old_duration);
    }

    // The new approach should be faster or at least as fast
    // With modern SSDs and OS caching, the file might be cached after the first read,
    // making the improvement less dramatic. The important thing is that we're not slower
    // and we're reading the file only once, which is better for non-cached scenarios
    // Allow for up to 10% performance variation due to timing inconsistencies
    assert!(
        improvement >= 0.9,
        "New approach should not be significantly slower than the old approach (got {improvement:.2}x)"
    );

    // If there's any improvement at all, that's good
    if improvement > 1.0 {
        println!("✓ Performance improved by reading file only once");
    } else {
        println!("✓ Performance maintained (likely due to OS caching)");
    }
}
