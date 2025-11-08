//! Integration tests for the file_processing module

use anidb_client_core::{
    HashAlgorithm, Progress,
    buffer::{DEFAULT_BUFFER_SIZE, get_memory_limit, memory_used},
    file_processing::{process_file, process_file_streaming},
};
use std::io::Write;
use tempfile::tempdir;
use tokio::sync::mpsc;

// Mutex to ensure tests run sequentially due to global memory tracking
use std::sync::LazyLock;
use tokio::sync::Mutex as AsyncMutex;
static TEST_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

#[tokio::test]
async fn test_process_large_file_memory_usage() {
    let _guard = TEST_MUTEX.lock().await;

    // Create a large test file (10MB)
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("large.bin");
    let mut file = std::fs::File::create(&file_path).unwrap();

    // Write 10MB of data
    let chunk = vec![0x42u8; 1_000_000]; // 1MB chunks
    for _ in 0..10 {
        file.write_all(&chunk).unwrap();
    }
    drop(file);

    let initial_memory = memory_used();

    // Process with multiple algorithms
    let algorithms = vec![
        HashAlgorithm::CRC32,
        HashAlgorithm::MD5,
        HashAlgorithm::SHA1,
    ];

    let (tx, mut rx) = mpsc::channel::<Progress>(100);

    // Track max memory usage
    let memory_task = tokio::spawn(async move {
        let mut max_memory = 0;
        while let Some(progress) = rx.recv().await {
            if let Some(memory) = progress.memory_usage_bytes {
                max_memory = max_memory.max(memory as usize);
            }
        }
        max_memory
    });

    let results = process_file(&file_path, &algorithms, Some(tx))
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    for result in &results {
        assert_eq!(result.input_size, 10_000_000);
    }

    let max_memory_used = memory_task.await.unwrap();

    // Memory used should be reasonable (buffer size + some overhead)
    // Note: max_memory_used already includes initial_memory since it tracks absolute memory usage
    assert!(max_memory_used <= DEFAULT_BUFFER_SIZE + 1_000_000); // 1MB overhead allowed

    // Final memory should be close to initial (buffers released)
    let final_memory = memory_used();
    assert!(final_memory <= initial_memory + 100_000); // 100KB overhead allowed
}

#[tokio::test]
async fn test_process_multiple_files_concurrently() {
    let _guard = TEST_MUTEX.lock().await;
    use futures::future::join_all;

    // Create multiple test files
    let dir = tempdir().unwrap();
    let mut file_paths = Vec::new();

    for i in 0..5 {
        let file_path = dir.path().join(format!("file{i}.txt"));
        let mut file = std::fs::File::create(&file_path).unwrap();
        write!(file, "Test file {i} with some content").unwrap();
        drop(file);
        file_paths.push(file_path);
    }

    let initial_memory = memory_used();

    // Process all files concurrently
    let tasks: Vec<_> = file_paths
        .iter()
        .map(|path| process_file(path, &[HashAlgorithm::CRC32], None))
        .collect();

    let all_results = join_all(tasks).await;

    // All should succeed
    assert_eq!(all_results.len(), 5);
    for result in all_results.iter() {
        let results = result.as_ref().unwrap();
        assert_eq!(results.len(), 1);
        // Each file has different content length
        assert!(results[0].input_size > 20);
    }

    // Memory should still be within limits
    assert!(memory_used() < get_memory_limit());

    // Memory should be mostly released
    let final_memory = memory_used();
    assert!(final_memory <= initial_memory + 500_000); // 500KB overhead for concurrent ops
}

#[tokio::test]
async fn test_custom_buffer_sizes() {
    let _guard = TEST_MUTEX.lock().await;
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.bin");
    let mut file = std::fs::File::create(&file_path).unwrap();

    // Create 100KB file
    let data = vec![0xAAu8; 100_000];
    file.write_all(&data).unwrap();
    drop(file);

    // Test with different buffer sizes
    let buffer_sizes = vec![1024, 4096, 16384, 65536];

    for buffer_size in buffer_sizes {
        let results =
            process_file_streaming(&file_path, &[HashAlgorithm::CRC32], buffer_size, None)
                .await
                .unwrap();

        assert_eq!(results[0].input_size, 100_000);
    }

    // After all operations, memory should still be reasonable
    assert!(memory_used() < get_memory_limit() / 10); // Should use less than 10% of limit
}

#[tokio::test]
async fn test_progress_reporting_accuracy() {
    let _guard = TEST_MUTEX.lock().await;
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("progress_test.bin");
    let mut file = std::fs::File::create(&file_path).unwrap();

    // Create 1MB file
    let data = vec![0xFFu8; 1_000_000];
    file.write_all(&data).unwrap();
    drop(file);

    let (tx, mut rx) = mpsc::channel(1000);

    let progress_task = tokio::spawn(async move {
        let mut updates = Vec::new();
        while let Some(progress) = rx.recv().await {
            updates.push(progress);
        }
        updates
    });

    let results = process_file_streaming(
        &file_path,
        &[HashAlgorithm::CRC32],
        8192, // 8KB buffer for more progress updates
        Some(tx),
    )
    .await
    .unwrap();

    let progress_updates = progress_task.await.unwrap();

    // Should have multiple progress updates
    assert!(progress_updates.len() > 10);

    // Progress should be monotonically increasing
    for window in progress_updates.windows(2) {
        assert!(window[1].bytes_processed >= window[0].bytes_processed);
        assert!(window[1].percentage >= window[0].percentage);
    }

    // Final progress should be 100%
    if let Some(last) = progress_updates.last() {
        assert!((last.percentage - 100.0).abs() < 0.1);
        assert_eq!(last.bytes_processed, 1_000_000);
    }

    assert_eq!(results[0].input_size, 1_000_000);
}

#[tokio::test]
async fn test_memory_limit_enforcement() {
    let _guard = TEST_MUTEX.lock().await;
    // This test verifies that we can't exceed the memory limit
    // even with concurrent operations
    use futures::future::join_all;

    let dir = tempdir().unwrap();
    let mut file_paths = Vec::new();

    // Create many files
    for i in 0..100 {
        let file_path = dir.path().join(format!("mem_test{i}.bin"));
        let mut file = std::fs::File::create(&file_path).unwrap();
        let data = vec![0u8; 100_000]; // 100KB each
        file.write_all(&data).unwrap();
        drop(file);
        file_paths.push(file_path);
    }

    // Try to process many files concurrently
    let tasks: Vec<_> = file_paths
        .iter()
        .map(|path| {
            // Use large buffers to try to exceed memory limit
            process_file_streaming(
                path,
                &[HashAlgorithm::CRC32],
                10 * 1024 * 1024, // 10MB buffer each
                None,
            )
        })
        .collect();

    // Some should succeed, some might fail due to memory limits
    let results = join_all(tasks).await;

    let successes = results.iter().filter(|r| r.is_ok()).count();
    let _failures = results.iter().filter(|r| r.is_err()).count();

    // At least some should succeed
    assert!(successes > 0);

    // If there are failures, they should be memory limit errors
    for result in results.iter().filter(|r| r.is_err()) {
        match result {
            Err(e) => {
                let error_string = format!("{e}");
                assert!(error_string.contains("Memory limit exceeded"));
            }
            _ => unreachable!(),
        }
    }

    // Total memory used should never exceed limit
    assert!(memory_used() <= get_memory_limit());
}
