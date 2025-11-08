//! File processing module using the new buffer management system
//!
//! This module provides streaming file processing with memory tracking

use crate::{
    Error, HashAlgorithm, HashResult, Progress, Result,
    buffer::{DEFAULT_BUFFER_SIZE, allocate_buffer, memory_used, release_buffer},
    error::IoError,
    hashing::{HashAlgorithmExt, StreamingHasher},
};
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::mpsc::Sender;

/// Process a file with the specified hash algorithms
pub async fn process_file(
    path: &Path,
    algorithms: &[HashAlgorithm],
    progress_sender: Option<Sender<Progress>>,
) -> Result<Vec<HashResult>> {
    // Check if file exists
    if !path.exists() {
        return Err(Error::Io(IoError::file_not_found(path)));
    }

    // Get file metadata
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|e| Error::Io(IoError::from_std(e).with_path(path)))?;
    let file_size = metadata.len();

    // Use default buffer size or smaller for small files
    let buffer_size = if file_size < DEFAULT_BUFFER_SIZE as u64 {
        file_size as usize
    } else {
        DEFAULT_BUFFER_SIZE
    };

    process_file_streaming(path, algorithms, buffer_size, progress_sender).await
}

/// Process a file chunk by chunk with memory tracking
pub async fn process_file_streaming(
    path: &Path,
    algorithms: &[HashAlgorithm],
    buffer_size: usize,
    progress_sender: Option<Sender<Progress>>,
) -> Result<Vec<HashResult>> {
    // Open the file
    let file = File::open(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::Io(IoError::file_not_found(path))
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            Error::Io(IoError::permission_denied(path, e))
        } else {
            Error::Io(IoError::from_std(e).with_path(path))
        }
    })?;

    let metadata = file
        .metadata()
        .await
        .map_err(|e| Error::Io(IoError::from_std(e).with_path(path)))?;
    let file_size = metadata.len();

    // Allocate buffer with memory tracking
    let buffer = allocate_buffer(buffer_size)?;

    // Initialize hashers for each algorithm
    let mut hashers: Vec<(HashAlgorithm, Box<dyn StreamingHasher>)> = Vec::new();
    for algorithm in algorithms {
        let algo_impl = algorithm.to_impl();
        let hasher = algo_impl.create_hasher();
        hashers.push((*algorithm, hasher));
    }

    // Create buffered reader
    let mut reader = BufReader::with_capacity(buffer.len(), file);
    let mut processing_buffer = buffer;

    let start_time = Instant::now();
    let mut bytes_processed = 0u64;

    // Process file in chunks
    loop {
        let bytes_read = reader
            .read(&mut processing_buffer)
            .await
            .map_err(|e| Error::Io(IoError::from_std(e).with_path(path)))?;

        if bytes_read == 0 {
            break;
        }

        // Update all hashers
        for (_, hasher) in &mut hashers {
            hasher.update(&processing_buffer[..bytes_read]);
        }

        bytes_processed += bytes_read as u64;

        // Send progress update if requested
        if let Some(ref sender) = progress_sender {
            let progress = Progress {
                percentage: (bytes_processed as f64 / file_size as f64) * 100.0,
                bytes_processed,
                total_bytes: file_size,
                throughput_mbps: (bytes_processed as f64 / (1024.0 * 1024.0))
                    / start_time.elapsed().as_secs_f64(),
                current_operation: "Hashing".to_string(),
                memory_usage_bytes: Some(memory_used() as u64),
                peak_memory_bytes: None,
                buffer_size: Some(buffer_size),
            };

            // Don't wait for send to complete
            let _ = sender.send(progress).await;
        }
    }

    // Release buffer
    release_buffer(processing_buffer);

    // Finalize all hashes
    let mut results = Vec::new();
    for (algorithm, hasher) in hashers {
        let hash = hasher.finalize();
        results.push(HashResult {
            algorithm,
            hash,
            input_size: bytes_processed,
            duration: start_time.elapsed(),
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use tokio::sync::mpsc;

    // Mutex to ensure tests run sequentially due to global memory tracking
    use std::sync::LazyLock;
    use tokio::sync::Mutex as AsyncMutex;
    static TEST_MUTEX: LazyLock<AsyncMutex<()>> = LazyLock::new(|| AsyncMutex::new(()));

    #[tokio::test]
    async fn test_process_small_file() {
        let _guard = TEST_MUTEX.lock().await;

        // Create test file
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"Hello, world!").unwrap();
        drop(file);

        // Process it
        let results = process_file(&file_path, &[HashAlgorithm::CRC32], None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].algorithm, HashAlgorithm::CRC32);
        assert_eq!(results[0].input_size, 13);
    }

    #[tokio::test]
    async fn test_process_with_progress() {
        let _guard = TEST_MUTEX.lock().await;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"Test data for progress").unwrap();
        drop(file);

        let (tx, mut rx) = mpsc::channel(10);

        // Spawn a task to collect progress updates
        let progress_task = tokio::spawn(async move {
            let mut updates = Vec::new();
            while let Some(progress) = rx.recv().await {
                updates.push(progress);
            }
            updates
        });

        let results = process_file(&file_path, &[HashAlgorithm::CRC32], Some(tx))
            .await
            .unwrap();

        // Wait for progress updates
        let updates = progress_task.await.unwrap();

        // Should have received progress updates
        assert!(!updates.is_empty());
        assert_eq!(results[0].algorithm, HashAlgorithm::CRC32);
    }

    #[tokio::test]
    async fn test_process_multiple_algorithms() {
        let _guard = TEST_MUTEX.lock().await;

        // Reset memory tracking
        #[cfg(any(test, feature = "test-internals"))]
        crate::buffer::reset_memory_tracking();

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(b"Multiple algorithm test").unwrap();
        drop(file);

        let algorithms = vec![
            HashAlgorithm::CRC32,
            HashAlgorithm::CRC32,
            HashAlgorithm::MD5,
        ];

        let results = process_file(&file_path, &algorithms, None).await.unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].algorithm, HashAlgorithm::CRC32);
        assert_eq!(results[1].algorithm, HashAlgorithm::CRC32);
        assert_eq!(results[2].algorithm, HashAlgorithm::MD5);

        // All should have processed the same number of bytes
        for result in &results {
            assert_eq!(result.input_size, 23);
        }
    }

    #[tokio::test]
    async fn test_process_file_not_found() {
        let _guard = TEST_MUTEX.lock().await;

        let result = process_file(
            Path::new("/nonexistent/file.txt"),
            &[HashAlgorithm::CRC32],
            None,
        )
        .await;

        assert!(result.is_err());
        match result {
            Err(Error::Io(ref io_err))
                if io_err.kind == crate::error::IoErrorKind::FileNotFound => {}
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_process_with_custom_buffer_size() {
        let _guard = TEST_MUTEX.lock().await;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        // Create a file larger than default buffer
        let data = vec![b'X'; 10_000];
        file.write_all(&data).unwrap();
        drop(file);

        // Process with small buffer
        let results = process_file_streaming(
            &file_path,
            &[HashAlgorithm::CRC32],
            1024, // 1KB buffer
            None,
        )
        .await
        .unwrap();

        assert_eq!(results[0].input_size, 10_000);
    }

    #[tokio::test]
    async fn test_memory_tracking_during_processing() {
        let _guard = TEST_MUTEX.lock().await;

        // Reset memory tracking
        #[cfg(any(test, feature = "test-internals"))]
        crate::buffer::reset_memory_tracking();

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(&vec![b'A'; 1_000_000]).unwrap(); // 1MB file
        drop(file);

        // Give time for any async operations to settle
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let initial_memory = memory_used();

        let (tx, mut rx) = mpsc::channel::<Progress>(100);

        // Spawn task to monitor memory usage
        let memory_task = tokio::spawn(async move {
            let mut max_memory = 0;
            while let Some(progress) = rx.recv().await {
                if let Some(memory) = progress.memory_usage_bytes {
                    max_memory = max_memory.max(memory as usize);
                }
            }
            max_memory
        });

        let _results = process_file(&file_path, &[HashAlgorithm::CRC32], Some(tx))
            .await
            .unwrap();

        let max_memory_used = memory_task.await.unwrap();

        // Memory should have been reported in progress
        // Even if global tracking shows low usage due to cleanup
        assert!(max_memory_used > 0, "Progress should report memory usage");

        // Give time for buffer to be properly released
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Check that memory was released
        let final_memory = memory_used();

        // Memory should be reasonably low after cleanup
        // Allow some overhead for system allocations
        assert!(
            final_memory <= initial_memory + 100_000, // Allow 100KB overhead
            "Memory should be released after processing: initial={initial_memory}, final={final_memory}"
        );
    }
}
