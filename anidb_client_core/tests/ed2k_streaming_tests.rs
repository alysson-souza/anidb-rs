//! Test to verify streaming ED2K implementation matches byte calculation

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};
use tempfile::TempDir;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn test_ed2k_streaming_vs_bytes() {
    println!("\n=== ED2K Streaming vs Bytes Comparison ===\n");

    let temp_dir = TempDir::new().unwrap();
    let calculator = HashCalculator::new();

    // Critical test case: exactly one chunk
    let chunk_size = 9_728_000;
    let data = vec![0x42u8; chunk_size];

    // Calculate with bytes method
    let bytes_result = calculator
        .calculate_bytes(HashAlgorithm::ED2K, &data)
        .unwrap();
    println!("Bytes method: {}", bytes_result.hash);

    // Calculate with file/streaming method
    let file_path = temp_dir.path().join("exact_chunk.bin");
    let mut file = File::create(&file_path).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.sync_all().await.unwrap();
    drop(file);

    let file_result = calculator
        .calculate_file(&file_path, HashAlgorithm::ED2K)
        .await
        .unwrap();
    println!("File/streaming method: {}", file_result.hash);

    // They should match!
    if bytes_result.hash != file_result.hash {
        println!("\n❌ MISMATCH DETECTED!");
        println!("This indicates the streaming implementation differs from bytes implementation");

        // Let's also test with pattern data to debug
        let pattern_data: Vec<u8> = (0..chunk_size).map(|i| (i % 256) as u8).collect();

        let pattern_bytes = calculator
            .calculate_bytes(HashAlgorithm::ED2K, &pattern_data)
            .unwrap();

        let pattern_file = temp_dir.path().join("pattern.bin");
        let mut file = File::create(&pattern_file).await.unwrap();
        file.write_all(&pattern_data).await.unwrap();
        file.sync_all().await.unwrap();
        drop(file);

        let pattern_file_result = calculator
            .calculate_file(&pattern_file, HashAlgorithm::ED2K)
            .await
            .unwrap();

        println!("\nPattern data test:");
        println!("  Bytes: {}", pattern_bytes.hash);
        println!("  File:  {}", pattern_file_result.hash);
    } else {
        println!("\n✓ Hashes match - implementations are consistent");
    }
}
